use std::cmp::min;

use ahash::AHashMap;
use alloy_rpc_types::Receipt;
use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CfgEnv, EVMError, Env, ResultAndState,
        SpecId::{self, LONDON},
        TransactTo, TxEnv, B256, U256,
    },
    Context, Database, Evm, EvmContext, Handler, L1BlockInfo,
};

use crate::{
    mv_memory::MvMemory, primitives::ChainSpec, EvmAccount, MemoryEntry, MemoryLocation,
    MemoryLocationHash, MemoryValue, ReadError, ReadLocations, ReadOrigin, ReadSet, Storage, TxIdx,
    TxVersion, WriteSet,
};

/// The execution error from the underlying EVM executor.
// Will there be DB errors outside of read?
pub type ExecutionError = EVMError<ReadError>;

/// Represents the state transitions of the EVM accounts after execution.
/// If the value is `None`, it indicates that the account is marked for removal.
/// If the value is `Some(new_state)`, it indicates that the account has become `new_state`.
type EvmStateTransitions = AHashMap<Address, Option<EvmAccount>>;

/// Execution result of a transaction
#[derive(Debug, Clone, PartialEq)]
pub struct PevmTxExecutionResult {
    /// Receipt of execution
    // TODO: Consider promoting to `ReceiptEnvelope` if there is high demand
    pub receipt: Receipt,
    /// State that got updated
    pub state: EvmStateTransitions,
}

impl PevmTxExecutionResult {
    /// Create a new execution from a raw REVM result.
    /// Note that `cumulative_gas_used` is preset to the gas used of this transaction.
    /// It should be post-processed with the remaining transactions in the block.
    pub fn from_revm(spec_id: SpecId, ResultAndState { result, state }: ResultAndState) -> Self {
        Self {
            receipt: Receipt {
                status: result.is_success().into(),
                cumulative_gas_used: result.gas_used() as u128,
                logs: result.into_logs(),
            },
            state: state
                .into_iter()
                .filter(|(_, account)| account.is_touched())
                .map(|(address, account)| {
                    if account.is_selfdestructed()
                    // https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-161.md
                    || account.is_empty() && spec_id.is_enabled_in(SpecId::SPURIOUS_DRAGON)
                    {
                        (address, None)
                    } else {
                        (address, Some(EvmAccount::from(account)))
                    }
                })
                .collect(),
        }
    }
}

pub(crate) enum VmExecutionResult {
    ReadError {
        blocking_tx_idx: TxIdx,
    },
    ExecutionError(ExecutionError),
    Ok {
        execution_result: PevmTxExecutionResult,
        read_locations: ReadLocations,
        write_set: WriteSet,
        // From which transaction index do we need to validate from after
        // this execution. This is `None` when no validation is required.
        // For instance, for transactions that only read and write to the
        // from and to addresses, which preprocessing has already ordered
        // dependencies correctly. Note that this is used to set the min
        // validation index in the scheduler, meaing a `None` here will
        // still be validated if there was a lower transaction that has
        // broken the preprocessed dependency chain and returned `Some`.
        // TODO: Better name & doc please.
        next_validation_idx: Option<TxIdx>,
    },
}

// A database interface that intercepts reads while executing a specific
// transaction with revm. It provides values from the multi-version data
// structure & storage, and tracks the read set of the current execution.
// TODO: Simplify this type, like grouping `from` and `to` into a
// `preprocessed_addresses` or a `preprocessed_locations` vector.
struct VmDb<'a, S: Storage> {
    vm: &'a Vm<'a, S>,
    tx_idx: &'a TxIdx,
    from: &'a Address,
    to: &'a Option<Address>,
    // List of memory locations that this transaction reads.
    read_set: ReadSet,
    // Check if this transaction has read anything other than its sender
    // and to accounts. We must validate from this transaction if it has.
    only_read_from_and_to: bool,
}

impl<'a, S: Storage> VmDb<'a, S> {
    fn new(
        vm: &'a Vm<'a, S>,
        tx_idx: &'a TxIdx,
        from: &'a Address,
        to: &'a Option<Address>,
    ) -> Self {
        Self {
            vm,
            tx_idx,
            from,
            to,
            only_read_from_and_to: true,
            read_set: ReadSet::default(),
        }
    }

    fn read(&mut self, location: MemoryLocation) -> Result<MemoryValue, ReadError> {
        let location_hash = self.vm.hasher.hash_one(location.clone());

        self.read_set.locations.insert(location_hash, vec![]);
        let read_origins = self.read_set.locations.get_mut(&location_hash).unwrap();

        if self.tx_idx == &0 {
            read_origins.push(ReadOrigin::Storage);
            return self.read_from_storage(&location, U256::ZERO);
        }

        // We enforce consecutive indexes for locations that all transactions write to like
        // the beneficiary balance. The goal is to not wastefully evaluate when we know
        // we're missing data -- let's just depend on the missing data instead.
        let need_consecutive_idxs = location_hash == self.vm.beneficiary_location_hash;
        // While we can depend on the precise missing transaction index (known during lazy evaluation),
        // through benchmark constantly retrying via the previous transaction index performs much better.
        let reschedule = Err(ReadError::BlockingIndex(self.tx_idx - 1));

        let Some(written_transactions) = self.vm.mv_memory.read_location(&location_hash) else {
            if need_consecutive_idxs {
                return reschedule;
            }
            read_origins.push(ReadOrigin::Storage);
            return self.read_from_storage(&location, U256::ZERO);
        };

        let mut total_addition = U256::ZERO;
        let mut current_idx = self.tx_idx;
        let mut iter = written_transactions.range(..current_idx);
        loop {
            match iter.next_back() {
                Some((blocking_idx, MemoryEntry::Estimate)) => {
                    return if need_consecutive_idxs {
                        reschedule
                    } else {
                        Err(ReadError::BlockingIndex(*blocking_idx))
                    }
                }
                Some((closest_idx, MemoryEntry::Data(tx_incarnation, value))) => {
                    if need_consecutive_idxs && closest_idx != &(current_idx - 1) {
                        return reschedule;
                    }
                    read_origins.push(ReadOrigin::MvMemory(TxVersion {
                        tx_idx: *closest_idx,
                        tx_incarnation: *tx_incarnation,
                    }));
                    match value {
                        MemoryValue::Basic(account) => {
                            let mut info = account.clone();
                            info.balance += total_addition;
                            break Ok(MemoryValue::Basic(info));
                        }
                        MemoryValue::LazyBalanceAddition(addition) => {
                            total_addition += addition;
                            current_idx = closest_idx;
                        }
                        storage => break Ok(storage.clone()),
                    }
                }
                _ => {
                    if need_consecutive_idxs && current_idx > &0 {
                        return reschedule;
                    }
                    read_origins.push(ReadOrigin::Storage);
                    return self.read_from_storage(&location, total_addition);
                }
            }
        }
    }

    fn read_from_storage(
        &self,
        location: &MemoryLocation,
        balance_addition: U256, // For lazy evaluation of atomically updated balances
    ) -> Result<MemoryValue, ReadError> {
        match location {
            MemoryLocation::Basic(address) => match self.vm.storage.basic(address) {
                Ok(Some(account)) => {
                    let mut info = AccountInfo::from(account);
                    info.balance += balance_addition;
                    Ok(MemoryValue::Basic(Box::new(info)))
                }
                Ok(None) => Err(ReadError::NotFound),
                Err(err) => Err(ReadError::StorageError(format!("{err:?}"))),
            },
            MemoryLocation::Storage(address, index) => self
                .vm
                .storage
                .storage(address, index)
                .map(MemoryValue::Storage)
                .map_err(|err| ReadError::StorageError(format!("{err:?}"))),
        }
    }
}

impl<'a, S: Storage> Database for VmDb<'a, S> {
    type Error = ReadError;

    // TODO: More granularity here to ensure we only record dependencies for,
    // for instance, only an account's balance instead of the whole account
    // info. That way we may also generalize beneficiary balance's lazy update
    // behaviour into `MemoryValue` for more use cases.
    fn basic(
        &mut self,
        address: Address,
        // TODO: Better way for REVM to notify explicit reads
        is_preload: bool,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        // We only return full accounts on explicit usage.
        if is_preload {
            return Ok(None);
        }
        // We return a mock for a non-contract recipient to avoid unncessarily
        // evaluating its balance here. Also skip transactions with the same from
        // & to until we have lazy updates for the sender nonce & balance.
        if &Some(address) == self.to && &address != self.from {
            // TODO: Live check for a contract deployed then used in the same block!
            let basic = self.vm.storage.basic(&address).unwrap();
            if basic.is_none() || basic.is_some_and(|basic| basic.code.is_none()) {
                return Ok(Some(AccountInfo {
                    // We need this hack to not flag this an empty account for
                    // destruction. Would definitely want a cleaner solution here.
                    nonce: 1,
                    ..AccountInfo::default()
                }));
            }
        }
        match self.read(MemoryLocation::Basic(address)) {
            Ok(MemoryValue::Basic(account)) => {
                if &address != self.from && &Some(address) != self.to {
                    self.only_read_from_and_to = false;
                }
                let info = *account;
                self.read_set
                    .accounts
                    // Avoid cloning the code as we can compare its hash
                    .insert(address, AccountInfo { code: None, ..info });
                Ok(Some(info))
            }
            Err(ReadError::NotFound) => Ok(None),
            Err(err) => Err(err),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.vm
            .storage
            .code_by_hash(&code_hash)
            .map(|code| code.map(Bytecode::from).unwrap_or_default())
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn has_storage(&mut self, address: Address) -> Result<bool, Self::Error> {
        self.vm
            .storage
            .has_storage(&address)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.only_read_from_and_to = false;
        match self.read(MemoryLocation::Storage(address, index)) {
            Err(err) => Err(err),
            Ok(MemoryValue::Storage(value)) => Ok(value),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.vm
            .storage
            .block_hash(&number)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }
}

// The VM describes how to read values to execute transactions. Also, it
// captures the read & write sets of each execution. Note that a single
// `Vm` can be shared among threads.
pub(crate) struct Vm<'a, S: Storage> {
    hasher: &'a ahash::RandomState,
    chain_spec: &'a ChainSpec,
    storage: &'a S,
    mv_memory: &'a MvMemory,
    spec_id: SpecId,
    block_env: BlockEnv,
    beneficiary_location_hash: MemoryLocationHash,
    txs: Vec<TxEnv>,
}

impl<'a, S: Storage> Vm<'a, S> {
    pub(crate) fn new(
        hasher: &'a ahash::RandomState,
        chain_spec: &'a ChainSpec,
        storage: &'a S,
        mv_memory: &'a MvMemory,
        spec_id: SpecId,
        block_env: BlockEnv,
        txs: Vec<TxEnv>,
    ) -> Self {
        Self {
            hasher,
            chain_spec,
            storage,
            mv_memory,
            spec_id,
            beneficiary_location_hash: hasher.hash_one(MemoryLocation::Basic(block_env.coinbase)),
            block_env,
            txs,
        }
    }

    // Execute a transaction. This can read from memory but cannot modify any state.
    // A successful execution returns:
    //   - A write-set consisting of memory locations and their updated values.
    //   - A read-set consisting of memory locations read during incarnation and its
    //   origin.
    //
    // An execution may observe a read dependency on a lower transaction. This happens
    // when the last incarnation of the dependency wrote to a memory location that
    // this transaction reads, but it aborted before the read. In this case, the
    // dependency index is returned via `blocking_tx_idx`. An execution task for this
    // transaction is re-scheduled after the blocking dependency finishes its
    // next incarnation.
    //
    // When a transaction attempts to write a value to a location, the location and
    // value are added to the write set, possibly replacing a pair with a prior value
    // (if it is not the first time the transaction wrote to this location during the
    // execution).
    pub(crate) fn execute(&self, tx_idx: TxIdx) -> VmExecutionResult {
        // SATEFY: A correct scheduler would guarantee this index to be inbound.
        let tx = unsafe { self.txs.get_unchecked(tx_idx) }.clone();
        let from = tx.caller;
        let (is_create_tx, to) = match tx.transact_to {
            TransactTo::Call(address) => (false, Some(address)),
            TransactTo::Create => (true, None),
        };
        let value = tx.value;

        // Set up DB
        let mut db = VmDb::new(self, &tx_idx, &from, &to);

        // Gas price
        let mut gas_price = if let Some(priority_fee) = tx.gas_priority_fee {
            min(tx.gas_price, priority_fee + self.block_env.basefee)
        } else {
            tx.gas_price
        };
        if self.spec_id.is_enabled_in(LONDON) {
            gas_price = gas_price.saturating_sub(self.block_env.basefee);
        }

        let is_deposit = match self.chain_spec {
            ChainSpec::Ethereum { .. } => false,
            #[cfg(feature = "optimism")]
            ChainSpec::Optimism { .. } => tx.optimism.source_hash.is_some(),
        };

        let enveloped_tx = match self.chain_spec {
            ChainSpec::Ethereum { .. } => None,
            #[cfg(feature = "optimism")]
            ChainSpec::Optimism { .. } => tx.optimism.enveloped_tx.clone(),
        };

        match execute_tx(
            &mut db,
            self.chain_spec,
            self.spec_id,
            self.block_env.clone(),
            tx,
            false,
        ) {
            Ok(result_and_state) => {
                let mut gas_payment =
                    Some(gas_price * U256::from(result_and_state.result.gas_used()));

                // There are at least three locations most of the time: the sender,
                // the recipient, and the beneficiary accounts.
                let mut write_set = WriteSet::with_capacity(3);
                for (address, account) in result_and_state.state.iter() {
                    if account.is_selfdestructed() {
                        write_set.push((
                            self.hasher.hash_one(MemoryLocation::Basic(*address)),
                            MemoryValue::Basic(Box::default()),
                        ));
                        continue;
                    }

                    if account.is_touched()
                        && db.read_set.accounts.get(address) != Some(&account.info)
                    {
                        // TODO: More granularity here to ensure we only notify new
                        // memory writes, for instance, only an account's balance instead
                        // of the whole account. That way we may also generalize beneficiary
                        // balance's lazy update behaviour into `MemoryValue` for more use cases.
                        // TODO: Confirm that we're not missing anything, like bytecode.
                        let mut account_info = account.info.clone();

                        let account_location_hash = if address == &self.block_env.coinbase {
                            account_info.balance += gas_payment.take().unwrap();
                            self.beneficiary_location_hash
                        } else {
                            self.hasher.hash_one(MemoryLocation::Basic(*address))
                        };

                        // Skip transactions with the same from & to until we have lazy updates
                        // for the sender nonce & balance.
                        if to == Some(*address)
                            && address != &from
                            && account.info.is_empty_code_hash()
                        {
                            write_set.push((
                                account_location_hash,
                                MemoryValue::LazyBalanceAddition(value),
                            ));
                        } else {
                            write_set.push((
                                account_location_hash,
                                MemoryValue::Basic(Box::new(account_info)),
                            ));
                        }
                    }

                    // TODO: We should move this to our read set like for account info?
                    for (slot, value) in account.changed_storage_slots() {
                        write_set.push((
                            self.hasher
                                .hash_one(MemoryLocation::Storage(*address, *slot)),
                            MemoryValue::Storage(value.present_value),
                        ));
                    }
                }

                // A non-existent explicit write hasn't taken the option.
                if let Some(gas_payment) = gas_payment {
                    write_set.push((
                        self.beneficiary_location_hash,
                        MemoryValue::LazyBalanceAddition(gas_payment),
                    ));
                }

                #[cfg(feature = "optimism")]
                {
                    if !is_deposit {
                        let l1_fee_recipient_address = revm::optimism::L1_FEE_RECIPIENT;
                        let l1_fee_recipient_location_hash = self
                            .hasher
                            .hash_one(MemoryLocation::Basic(l1_fee_recipient_address));

                        let base_fee_vault_address = revm::optimism::BASE_FEE_RECIPIENT;
                        let base_fee_vault_location_hash = self
                            .hasher
                            .hash_one(MemoryLocation::Basic(base_fee_vault_address));

                        let l1_block_info = match L1BlockInfo::try_fetch(&mut db, self.spec_id) {
                            Ok(value) => value,
                            Err(error) => {
                                return VmExecutionResult::ExecutionError(EVMError::Database(
                                    error,
                                ));
                            }
                        };

                        let Some(enveloped_tx) = &enveloped_tx else {
                            return VmExecutionResult::ExecutionError(EVMError::Custom(
                                "[OPTIMISM] Failed to load enveloped transaction.".to_string(),
                            ));
                        };

                        let l1_cost =
                            l1_block_info.calculate_tx_l1_cost(enveloped_tx, self.spec_id);

                        write_set.push((
                            l1_fee_recipient_location_hash,
                            MemoryValue::LazyBalanceAddition(l1_cost),
                        ));

                        write_set.push((
                            base_fee_vault_location_hash,
                            MemoryValue::LazyBalanceAddition(
                                self.block_env.basefee
                                    * U256::from(result_and_state.result.gas_used()),
                            ),
                        ));
                    }
                }

                let mut next_validation_idx = None;
                if tx_idx > 0 {
                    // Validate from this transaction if it reads something outside of its
                    // sender and to infos.
                    if !db.only_read_from_and_to {
                        next_validation_idx = Some(tx_idx);
                    }
                    // Validate from the next transaction if doesn't read externally but
                    // deploy a new contract.
                    else if is_create_tx {
                        next_validation_idx = Some(tx_idx + 1);
                    }
                    // Validate from the next transaction if it writes to a location outside
                    // of the beneficiary account, its sender and to infos.
                    else {
                        let from_hash = self.hasher.hash_one(from);
                        let to_hash = self.hasher.hash_one(to.unwrap());
                        if write_set.iter().any(|(location_hash, _)| {
                            location_hash != &from_hash
                                && location_hash != &to_hash
                                && location_hash != &self.beneficiary_location_hash
                        }) {
                            next_validation_idx = Some(tx_idx + 1);
                        }
                    }
                }

                VmExecutionResult::Ok {
                    execution_result: PevmTxExecutionResult::from_revm(
                        self.spec_id,
                        result_and_state,
                    ),
                    read_locations: db.read_set.locations,
                    write_set,
                    next_validation_idx,
                }
            }
            Err(EVMError::Database(ReadError::BlockingIndex(blocking_tx_idx))) => {
                VmExecutionResult::ReadError { blocking_tx_idx }
            }
            Err(err) => VmExecutionResult::ExecutionError(err),
        }
    }
}

// TODO: Move to better place?
pub(crate) fn execute_tx<DB: Database>(
    db: DB,
    chain_spec: &ChainSpec,
    spec_id: SpecId,
    block_env: BlockEnv,
    tx: TxEnv,
    with_reward_beneficiary: bool,
) -> Result<ResultAndState, EVMError<DB::Error>> {
    // This is much uglier than the builder interface but can be up to 50% faster!!
    let context = Context {
        evm: EvmContext::new_with_env(
            db,
            Env::boxed(
                CfgEnv::default().with_chain_id(chain_spec.chain_id()),
                block_env.clone(),
                tx,
            ),
        ),
        external: (),
    };
    // TODO: Support OP handlers
    let handler = match chain_spec {
        ChainSpec::Ethereum { .. } => Handler::mainnet_with_spec(spec_id, with_reward_beneficiary),
        #[cfg(feature = "optimism")]
        ChainSpec::Optimism { .. } => Handler::optimism_with_spec(spec_id, with_reward_beneficiary),
    };
    Evm::new(context, handler).transact()
}
