use std::{cmp::min, sync::Arc};

use ahash::AHashMap;
use alloy_rpc_types::Receipt;
use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CfgEnv, EVMError, Env, ResultAndState,
        SpecId::{self, LONDON},
        TransactTo, TxEnv, B256, U256,
    },
    Context, Database, Evm, EvmContext, Handler,
};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    EvmAccount, MemoryEntry, MemoryLocation, MemoryValue, ReadError, ReadOrigin, ReadSet, Storage,
    TxIdx, WriteSet,
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
        read_set: ReadSet,
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
    // References from the main VM instance.
    beneficiary_location: &'a MemoryLocation,
    tx_idx: &'a TxIdx,
    from: &'a Address,
    to: &'a Option<Address>,
    mv_memory: &'a Arc<MvMemory>,
    storage: &'a S,
    // List of memory locations that this transaction reads.
    read_set: ReadSet,
    // Check if this transaction has read an account other than its sender
    // and to addresses. We must validate from this transaction if it has.
    read_externally: bool,
}

impl<'a, S: Storage> VmDb<'a, S> {
    fn new(
        beneficiary_location: &'a MemoryLocation,
        tx_idx: &'a TxIdx,
        from: &'a Address,
        to: &'a Option<Address>,
        mv_memory: &'a Arc<MvMemory>,
        storage: &'a S,
    ) -> Self {
        Self {
            beneficiary_location,
            tx_idx,
            mv_memory,
            storage,
            read_set: ReadSet {
                // There are at least two locations most of the time: the sender
                // and the recipient accounts.
                common: Vec::with_capacity(2),
                beneficiary: Vec::new(),
            },
            from,
            to,
            read_externally: false,
        }
    }

    fn read(
        &mut self,
        location: MemoryLocation,
        update_read_set: bool,
    ) -> Result<MemoryValue, ReadError> {
        if &location == self.beneficiary_location {
            return self.read_beneficiary();
        }

        match self.mv_memory.read_closest(&location, self.tx_idx) {
            ReadMemoryResult::ReadError { blocking_tx_idx } => {
                Err(ReadError::BlockingIndex(blocking_tx_idx))
            }
            ReadMemoryResult::NotFound => {
                if update_read_set {
                    self.read_set
                        .common
                        .push((location.clone(), ReadOrigin::Storage));
                }
                match location {
                    MemoryLocation::Basic(address) => match self.storage.basic(&address) {
                        Ok(Some(account)) => Ok(MemoryValue::Basic(Box::new(account.into()))),
                        Ok(None) => Err(ReadError::NotFound),
                        Err(err) => Err(ReadError::StorageError(format!("{err:?}"))),
                    },
                    MemoryLocation::Storage(address, index) => self
                        .storage
                        .storage(&address, &index)
                        .map(MemoryValue::Storage)
                        .map_err(|err| ReadError::StorageError(format!("{err:?}"))),
                }
            }
            ReadMemoryResult::Ok { version, value } => {
                if update_read_set {
                    self.read_set
                        .common
                        .push((location, ReadOrigin::MvMemory(version)));
                }
                Ok(value)
            }
        }
    }

    fn read_beneficiary(&mut self) -> Result<MemoryValue, ReadError> {
        if *self.tx_idx == 0 {
            return Ok(MemoryValue::Basic(Box::new(
                self.read_beneficiary_from_storage()?,
            )));
        }

        // We simply register this transaction as dependency of the previous in
        // the racing cases that the previous transactions aren't yet ready.
        // TODO: Only reschedule up to a certain number of times.
        let reschedule = Err(ReadError::BlockingIndex(self.tx_idx - 1));

        let Some(written_beneficiary) = self.mv_memory.read_beneficiary() else {
            return reschedule;
        };

        let mut balance_addition = U256::ZERO;
        let mut tx_idx = self.tx_idx - 1;
        loop {
            match written_beneficiary.get(&tx_idx) {
                Some(MemoryEntry::Data(tx_incarnation, value)) => {
                    self.read_set
                        .beneficiary
                        .push(ReadOrigin::MvMemory(crate::TxVersion {
                            tx_idx,
                            tx_incarnation: *tx_incarnation,
                        }));
                    match value {
                        MemoryValue::Basic(account) => {
                            let mut account = account.clone();
                            account.balance += balance_addition;
                            return Ok(MemoryValue::Basic(account));
                        }
                        MemoryValue::LazyBeneficiaryBalance(addition) => {
                            // TODO: Be careful with overflows
                            balance_addition += addition;
                        }
                        _ => unreachable!("Unexpected storage value for beneficiary account info"),
                    }
                }
                _ => return reschedule,
            }
            if tx_idx == 0 {
                let mut account = self.read_beneficiary_from_storage()?;
                account.balance += balance_addition;
                return Ok(MemoryValue::Basic(Box::new(account)));
            }
            tx_idx -= 1;
        }
    }

    fn read_beneficiary_from_storage(&self) -> Result<AccountInfo, ReadError> {
        match self.storage.basic(self.beneficiary_location.address()) {
            Ok(Some(account)) => Ok(account.into()),
            Ok(None) => Err(ReadError::NotFound),
            Err(err) => Err(ReadError::StorageError(format!("{err:?}"))),
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
        // We preload a mock beneficiary account, to only lazy evaluate it on
        // explicit reads and once BlockSTM is completed.
        if &address == self.beneficiary_location.address() && is_preload {
            return Ok(Some(AccountInfo::default()));
        }
        match self.read(MemoryLocation::Basic(address), !is_preload) {
            Ok(MemoryValue::Basic(account)) => {
                if !is_preload && &address != self.from && &Some(address) != self.to {
                    self.read_externally = true;
                }
                Ok(Some(*account))
            }
            Err(ReadError::NotFound) => Ok(None),
            Err(err) => Err(err),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.storage
            .code_by_hash(&code_hash)
            .map(|code| code.map(Bytecode::from).unwrap_or_default())
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn has_storage(&mut self, address: Address) -> Result<bool, Self::Error> {
        self.storage
            .has_storage(&address)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.read_externally = true;
        match self.read(MemoryLocation::Storage(address, index), true) {
            Err(err) => Err(err),
            Ok(MemoryValue::Storage(value)) => Ok(value),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.storage
            .block_hash(&number)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }
}

// The VM describes how to read values to execute transactions. Also, it
// captures the read & write sets of each execution. Note that a single
// `Vm` can be shared among threads.
pub(crate) struct Vm<S: Storage> {
    spec_id: SpecId,
    block_env: BlockEnv,
    beneficiary_location: MemoryLocation,
    txs: Vec<TxEnv>,
    storage: S,
    mv_memory: Arc<MvMemory>,
}

impl<S: Storage> Vm<S> {
    pub(crate) fn new(
        spec_id: SpecId,
        block_env: BlockEnv,
        txs: Vec<TxEnv>,
        storage: S,
        mv_memory: Arc<MvMemory>,
    ) -> Self {
        Self {
            spec_id,
            beneficiary_location: MemoryLocation::Basic(block_env.coinbase),
            block_env,
            txs,
            storage,
            mv_memory,
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
        let (is_call_tx, to) = match tx.transact_to {
            TransactTo::Call(address) => (false, Some(address)),
            TransactTo::Create => (true, None),
        };

        // Set up DB
        let mut db = VmDb::new(
            &self.beneficiary_location,
            &tx_idx,
            &from,
            &to,
            &self.mv_memory,
            &self.storage,
        );

        // Gas price
        let mut gas_price = if let Some(priority_fee) = tx.gas_priority_fee {
            min(tx.gas_price, priority_fee + self.block_env.basefee)
        } else {
            tx.gas_price
        };
        if self.spec_id.is_enabled_in(LONDON) {
            gas_price = gas_price.saturating_sub(self.block_env.basefee);
        }
        match execute_tx(&mut db, self.spec_id, self.block_env.clone(), tx, false) {
            Ok(result_and_state) => {
                let mut gas_payment =
                    Some(gas_price * U256::from(result_and_state.result.gas_used()));

                // There are at least three locations most of the time: the sender,
                // the recipient, and the beneficiary accounts.
                let mut write_set = AHashMap::<MemoryLocation, MemoryValue>::with_capacity(3);
                for (address, account) in result_and_state.state.iter() {
                    // TODO: Port this change check to our read set instead of REVM.
                    // We then let `execute_tx` output `PevmTxExecutionResult` directly and handle
                    // changes based on those processed transisions. Let make sure we handle
                    // removed accounts correctly afterwards.
                    if account.is_info_changed() {
                        // TODO: More granularity here to ensure we only notify new
                        // memory writes, for instance, only an account's balance instead
                        // of the whole account. That way we may also generalize beneficiary
                        // balance's lazy update behaviour into `MemoryValue` for more use cases.
                        // TODO: Confirm that we're not missing anything, like bytecode.
                        let mut account_info = account.info.clone();
                        if address == &self.block_env.coinbase {
                            account_info.balance += gas_payment.take().unwrap();
                        }

                        // We must reset the original values for the incoming reads to detect changes.
                        // TODO: Port this logic to PEVM's read & write sets instead of REVM.
                        account_info.previous_or_original_balance = account_info.balance;
                        account_info.previous_or_original_code_hash = account_info.code_hash;
                        account_info.previous_or_original_nonce = account_info.nonce;

                        write_set.insert(
                            MemoryLocation::Basic(*address),
                            MemoryValue::Basic(Box::new(account_info)),
                        );
                    }
                    for (slot, value) in account.changed_storage_slots() {
                        write_set.insert(
                            MemoryLocation::Storage(*address, *slot),
                            MemoryValue::Storage(value.present_value),
                        );
                    }
                }

                // A non-existent explicit write hasn't taken the option.
                if let Some(gas_payment) = gas_payment {
                    write_set.insert(
                        self.beneficiary_location.clone(),
                        MemoryValue::LazyBeneficiaryBalance(gas_payment),
                    );
                }

                let mut next_validation_idx = None;
                if tx_idx > 0 {
                    // Validate from this transaction if it reads something outside of its
                    // sender and to infos.
                    if db.read_externally {
                        next_validation_idx = Some(tx_idx);
                    }
                    // Validate from the next transaction if doesn't read externally but
                    // deploy a new contract.
                    else if is_call_tx {
                        next_validation_idx = Some(tx_idx + 1);
                    }
                    // Validate from the next transaction if it writes to a location outside
                    // of the beneficiary account, its sender and to infos.
                    else {
                        let to = to.unwrap();
                        if write_set.iter().any(|(location, _)| {
                            let address = location.address();
                            address != &from
                                && address != &to
                                && address != &self.block_env.coinbase
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
                    read_set: db.read_set,
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
// TODO: Convert the output `ResultAndState` to `PevmTxExecutionResult` for a
// much cleaner interface. We currently need `ResultAndState` for the parallel
// executor to check for changed accounts via REVM. Can pull this off once that
// check is moved to our own read set.
pub(crate) fn execute_tx<DB: Database>(
    db: DB,
    spec_id: SpecId,
    block_env: BlockEnv,
    tx: TxEnv,
    with_reward_beneficiary: bool,
) -> Result<ResultAndState, EVMError<DB::Error>> {
    // This is much uglier than the builder interface but can be up to 50% faster!!
    let context = Context {
        evm: EvmContext::new_with_env(db, Env::boxed(CfgEnv::default(), block_env.clone(), tx)),
        external: (),
    };
    // TODO: Support OP handlers
    let handler = Handler::mainnet_with_spec(spec_id, with_reward_beneficiary);
    Evm::new(context, handler).transact()
}
