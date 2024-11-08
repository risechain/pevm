use alloy_primitives::TxKind;
use alloy_rpc_types::Receipt;
use hashbrown::HashMap;
use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CfgEnv, EVMError, Env, InvalidTransaction,
        ResultAndState, SpecId, TxEnv, B256, KECCAK_EMPTY, U256,
    },
    Context, Database, Evm, EvmContext,
};
use smallvec::{smallvec, SmallVec};

use crate::{
    chain::{PevmChain, RewardPolicy},
    hash_determinisitic,
    mv_memory::MvMemory,
    storage::BytecodeConversionError,
    AccountBasic, BuildIdentityHasher, BuildSuffixHasher, EvmAccount, FinishExecFlags, MemoryEntry,
    MemoryLocation, MemoryLocationHash, MemoryValue, ReadOrigin, ReadOrigins, ReadSet, Storage,
    TxIdx, TxVersion, WriteSet,
};

/// The execution error from the underlying EVM executor.
// Will there be DB errors outside of read?
pub type ExecutionError = EVMError<ReadError>;

/// Represents the state transitions of the EVM accounts after execution.
/// If the value is [None], it indicates that the account is marked for removal.
/// If the value is [`Some(new_state)`], it indicates that the account has become [`new_state`].
type EvmStateTransitions = HashMap<Address, Option<EvmAccount>, BuildSuffixHasher>;

/// Execution result of a transaction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PevmTxExecutionResult {
    /// Receipt of execution
    // TODO: Consider promoting to [ReceiptEnvelope] if there is high demand
    pub receipt: Receipt,
    /// State that got updated
    pub state: EvmStateTransitions,
}

impl PevmTxExecutionResult {
    /// Construct a Pevm execution result from a raw Revm result.
    /// Note that [`cumulative_gas_used`] is preset to the gas used of this transaction.
    /// It should be post-processed with the remaining transactions in the block.
    pub fn from_revm<C: PevmChain>(
        chain: &C,
        spec_id: SpecId,
        ResultAndState { result, state }: ResultAndState,
    ) -> Self {
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
                        || account.is_empty() && chain.is_eip_161_enabled(spec_id)
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

pub(crate) enum VmExecutionError {
    Retry,
    FallbackToSequential,
    Blocking(TxIdx),
    ExecutionError(ExecutionError),
}

/// Errors when reading a memory location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Cannot read memory location from storage.
    StorageError(String),
    /// This memory location has been written by a lower transaction.
    Blocking(TxIdx),
    /// There has been an inconsistent read like reading the same
    /// location from storage in the first call but from [`VmMemory`] in
    /// the next.
    InconsistentRead,
    /// Found an invalid nonce, like the first transaction of a sender
    /// not having a (+1) nonce from storage.
    InvalidNonce(TxIdx),
    /// Read a self-destructed account that is very hard to handle, as
    /// there is no performant way to mark all storage slots as cleared.
    SelfDestructedAccount,
    /// The bytecode is invalid and cannot be converted.
    InvalidBytecode(BytecodeConversionError),
    /// The stored memory value type doesn't match its location type.
    /// TODO: Handle this at the type level?
    InvalidMemoryValueType,
}

impl From<ReadError> for VmExecutionError {
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::InconsistentRead => Self::Retry,
            ReadError::SelfDestructedAccount => Self::FallbackToSequential,
            ReadError::Blocking(tx_idx) => Self::Blocking(tx_idx),
            _ => Self::ExecutionError(EVMError::Database(err)),
        }
    }
}

pub(crate) struct VmExecutionResult {
    pub(crate) execution_result: PevmTxExecutionResult,
    pub(crate) flags: FinishExecFlags,
}

// A database interface that intercepts reads while executing a specific
// transaction with Revm. It provides values from the multi-version data
// structure & storage, and tracks the read set of the current execution.
struct VmDb<'a, S: Storage, C: PevmChain> {
    vm: &'a Vm<'a, S, C>,
    tx_idx: TxIdx,
    tx: &'a TxEnv,
    from_hash: MemoryLocationHash,
    to_hash: Option<MemoryLocationHash>,
    to_code_hash: Option<B256>,
    // Indicates if we lazy update this transaction.
    // Only applied to raw transfers' senders & recipients at the moment.
    is_lazy: bool,
    read_set: ReadSet,
    // TODO: Clearer type for [AccountBasic] plus code hash
    read_accounts: HashMap<MemoryLocationHash, (AccountBasic, Option<B256>), BuildIdentityHasher>,
}

impl<'a, S: Storage, C: PevmChain> VmDb<'a, S, C> {
    fn new(
        vm: &'a Vm<'a, S, C>,
        tx_idx: TxIdx,
        tx: &'a TxEnv,
        from_hash: MemoryLocationHash,
        to_hash: Option<MemoryLocationHash>,
    ) -> Result<Self, ReadError> {
        let mut db = Self {
            vm,
            tx_idx,
            tx,
            from_hash,
            to_hash,
            to_code_hash: None,
            is_lazy: false,
            // Unless it is a raw transfer that is lazy updated, we'll
            // read at least from the sender and recipient accounts.
            read_set: ReadSet::with_capacity_and_hasher(2, BuildIdentityHasher::default()),
            read_accounts: HashMap::with_capacity_and_hasher(2, BuildIdentityHasher::default()),
        };
        // We only lazy update raw transfers that already have the sender
        // or recipient in [MvMemory] since sequentially evaluating memory
        // locations with only one entry is much costlier than fully
        // evaluating it concurrently.
        // TODO: Only lazy update in block syncing mode, not for block
        // building.
        if let TxKind::Call(to) = tx.transact_to {
            db.to_code_hash = db.get_code_hash(to)?;
            db.is_lazy = db.to_code_hash.is_none()
                && (vm.mv_memory.data.contains_key(&from_hash)
                    || vm.mv_memory.data.contains_key(&to_hash.unwrap()));
            if to != vm.block_env.coinbase && !db.is_lazy && vm.mv_memory.is_lazy(&to) {
                vm.mv_memory.remove_lazy_address(&to);
            }
        }
        Ok(db)
    }

    fn hash_basic(&self, address: &Address) -> MemoryLocationHash {
        if address == &self.tx.caller {
            return self.from_hash;
        }
        if let TxKind::Call(to) = &self.tx.transact_to {
            if to == address {
                return self.to_hash.unwrap();
            }
        }
        hash_determinisitic(MemoryLocation::Basic(*address))
    }

    // Push a new read origin. Return an error when there's already
    // an origin but doesn't match the new one to force re-execution.
    fn push_origin(read_origins: &mut ReadOrigins, origin: ReadOrigin) -> Result<(), ReadError> {
        if let Some(prev_origin) = read_origins.last() {
            if prev_origin != &origin {
                return Err(ReadError::InconsistentRead);
            }
        } else {
            read_origins.push(origin);
        }
        Ok(())
    }

    fn get_code_hash(&mut self, address: Address) -> Result<Option<B256>, ReadError> {
        let location_hash = hash_determinisitic(MemoryLocation::CodeHash(address));
        let read_origins = self.read_set.entry(location_hash).or_default();

        // Try to read the latest code hash in [MvMemory]
        // TODO: Memoize read locations (expected to be small) here in [Vm] to avoid
        // contention in [MvMemory]
        if let Some(written_transactions) = self.vm.mv_memory.data.get(&location_hash) {
            if let Some((tx_idx, MemoryEntry::Data(tx_incarnation, value))) =
                written_transactions.range(..self.tx_idx).next_back()
            {
                match value {
                    MemoryValue::SelfDestructed => {
                        return Err(ReadError::SelfDestructedAccount);
                    }
                    MemoryValue::CodeHash(code_hash) => {
                        Self::push_origin(
                            read_origins,
                            ReadOrigin::MvMemory(TxVersion {
                                tx_idx: *tx_idx,
                                tx_incarnation: *tx_incarnation,
                            }),
                        )?;
                        return Ok(Some(*code_hash));
                    }
                    _ => {}
                }
            }
        };

        // Fallback to storage
        Self::push_origin(read_origins, ReadOrigin::Storage)?;
        self.vm
            .storage
            .code_hash(&address)
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }
}

impl<'a, S: Storage, C: PevmChain> Database for VmDb<'a, S, C> {
    type Error = ReadError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let location_hash = self.hash_basic(&address);

        // We return a mock for non-contract addresses (for lazy updates) to avoid
        // unnecessarily evaluating its balance here.
        if self.is_lazy {
            if location_hash == self.from_hash {
                return Ok(Some(AccountInfo {
                    nonce: self.tx.nonce.unwrap_or(1),
                    balance: U256::MAX,
                    code: None,
                    code_hash: KECCAK_EMPTY,
                }));
            } else if Some(location_hash) == self.to_hash {
                return Ok(None);
            }
        }

        let read_origins = self.read_set.entry(location_hash).or_default();
        let has_prev_origins = !read_origins.is_empty();
        // We accumulate new origins to either:
        // - match with the previous origins to check consistency
        // - register origins on the first read
        let mut new_origins = SmallVec::new();

        let mut final_account = None;
        let mut balance_addition = U256::ZERO;
        // The sign of [balance_addition] since it can be negative for lazy senders.
        let mut positive_addition = true;
        let mut nonce_addition = 0;

        // Try reading from multi-version data
        if self.tx_idx > 0 {
            if let Some(written_transactions) = self.vm.mv_memory.data.get(&location_hash) {
                let mut iter = written_transactions.range(..self.tx_idx);

                // Fully evaluate lazy updates
                loop {
                    match iter.next_back() {
                        Some((blocking_idx, MemoryEntry::Estimate)) => {
                            return Err(ReadError::Blocking(*blocking_idx))
                        }
                        Some((closest_idx, MemoryEntry::Data(tx_incarnation, value))) => {
                            // About to push a new origin
                            // Inconsistent: new origin will be longer than the previous!
                            if has_prev_origins && read_origins.len() == new_origins.len() {
                                return Err(ReadError::InconsistentRead);
                            }
                            let origin = ReadOrigin::MvMemory(TxVersion {
                                tx_idx: *closest_idx,
                                tx_incarnation: *tx_incarnation,
                            });
                            // Inconsistent: new origin is different from the previous!
                            if has_prev_origins
                                && unsafe { read_origins.get_unchecked(new_origins.len()) }
                                    != &origin
                            {
                                return Err(ReadError::InconsistentRead);
                            }
                            new_origins.push(origin);
                            match value {
                                MemoryValue::Basic(basic) => {
                                    // TODO: Return [SelfDestructedAccount] if [basic] is
                                    // [SelfDestructed]?
                                    // For now we are betting on [code_hash] triggering the
                                    // sequential fallback when we read a self-destructed contract.
                                    final_account = Some(basic.clone());
                                    break;
                                }
                                MemoryValue::LazyRecipient(addition) => {
                                    if positive_addition {
                                        balance_addition =
                                            balance_addition.saturating_add(*addition);
                                    } else {
                                        positive_addition = *addition >= balance_addition;
                                        balance_addition = balance_addition.abs_diff(*addition);
                                    }
                                }
                                MemoryValue::LazySender(subtraction) => {
                                    if positive_addition {
                                        positive_addition = balance_addition >= *subtraction;
                                        balance_addition = balance_addition.abs_diff(*subtraction);
                                    } else {
                                        balance_addition =
                                            balance_addition.saturating_add(*subtraction);
                                    }
                                    nonce_addition += 1;
                                }
                                _ => return Err(ReadError::InvalidMemoryValueType),
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        // Fall back to storage
        if final_account.is_none() {
            // Populate [Storage] on the first read
            if !has_prev_origins {
                new_origins.push(ReadOrigin::Storage);
            }
            // Inconsistent: previous origin is longer or didn't read
            // from storage for the last origin.
            else if read_origins.len() != new_origins.len() + 1
                || read_origins.last() != Some(&ReadOrigin::Storage)
            {
                return Err(ReadError::InconsistentRead);
            }
            final_account = match self.vm.storage.basic(&address) {
                Ok(Some(basic)) => Some(basic),
                Ok(None) => (balance_addition > U256::ZERO).then(AccountBasic::default),
                Err(err) => return Err(ReadError::StorageError(err.to_string())),
            };
        }

        // Populate read origins on the first read.
        // Otherwise [read_origins] matches [new_origins] already.
        if !has_prev_origins {
            *read_origins = new_origins;
        }

        if let Some(mut account) = final_account {
            // Check sender nonce
            account.nonce += nonce_addition;
            if location_hash == self.from_hash
                && self.tx.nonce.is_some_and(|nonce| nonce != account.nonce)
            {
                return if self.tx_idx > 0 {
                    // TODO: Better retry strategy -- immediately, to the
                    // closest sender tx, to the missing sender tx, etc.
                    Err(ReadError::Blocking(self.tx_idx - 1))
                } else {
                    Err(ReadError::InvalidNonce(self.tx_idx))
                };
            }

            // Fully evaluate the account and register it to read cache
            // to later check if they have changed (been written to).
            if positive_addition {
                account.balance = account.balance.saturating_add(balance_addition);
            } else {
                account.balance = account.balance.saturating_sub(balance_addition);
            };

            let code_hash = if Some(location_hash) == self.to_hash {
                self.to_code_hash
            } else {
                self.get_code_hash(address)?
            };
            let code = if let Some(code_hash) = &code_hash {
                if let Some(code) = self.vm.mv_memory.new_bytecodes.get(code_hash) {
                    Some(code.clone())
                } else {
                    match self.vm.storage.code_by_hash(code_hash) {
                        Ok(code) => code
                            .map(Bytecode::try_from)
                            .transpose()
                            .map_err(ReadError::InvalidBytecode)?,
                        Err(err) => return Err(ReadError::StorageError(err.to_string())),
                    }
                }
            } else {
                None
            };
            self.read_accounts
                .insert(location_hash, (account.clone(), code_hash));

            return Ok(Some(AccountInfo {
                balance: account.balance,
                nonce: account.nonce,
                code_hash: code_hash.unwrap_or(KECCAK_EMPTY),
                code,
            }));
        }

        Ok(None)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self
            .vm
            .storage
            .code_by_hash(&code_hash)
            .map_err(|err| ReadError::StorageError(err.to_string()))?
        {
            Some(evm_code) => Bytecode::try_from(evm_code).map_err(ReadError::InvalidBytecode),
            None => Ok(Bytecode::default()),
        }
    }

    fn has_storage(&mut self, address: Address) -> Result<bool, Self::Error> {
        self.vm
            .storage
            .has_storage(&address)
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let location_hash = hash_determinisitic(MemoryLocation::Storage(address, index));

        let read_origins = self.read_set.entry(location_hash).or_default();

        // Try reading from multi-version data
        if self.tx_idx > 0 {
            if let Some(written_transactions) = self.vm.mv_memory.data.get(&location_hash) {
                if let Some((closest_idx, entry)) =
                    written_transactions.range(..self.tx_idx).next_back()
                {
                    match entry {
                        MemoryEntry::Data(tx_incarnation, MemoryValue::Storage(value)) => {
                            Self::push_origin(
                                read_origins,
                                ReadOrigin::MvMemory(TxVersion {
                                    tx_idx: *closest_idx,
                                    tx_incarnation: *tx_incarnation,
                                }),
                            )?;
                            return Ok(*value);
                        }
                        MemoryEntry::Estimate => return Err(ReadError::Blocking(*closest_idx)),
                        _ => return Err(ReadError::InvalidMemoryValueType),
                    }
                }
            }
        }

        // Fall back to storage
        Self::push_origin(read_origins, ReadOrigin::Storage)?;
        self.vm
            .storage
            .storage(&address, &index)
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.vm
            .storage
            .block_hash(&number)
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }
}

pub(crate) struct Vm<'a, S: Storage, C: PevmChain> {
    storage: &'a S,
    mv_memory: &'a MvMemory,
    chain: &'a C,
    block_env: &'a BlockEnv,
    txs: &'a [TxEnv],
    spec_id: SpecId,
    beneficiary_location_hash: MemoryLocationHash,
    reward_policy: RewardPolicy,
}

impl<'a, S: Storage, C: PevmChain> Vm<'a, S, C> {
    pub(crate) fn new(
        storage: &'a S,
        mv_memory: &'a MvMemory,
        chain: &'a C,
        block_env: &'a BlockEnv,
        txs: &'a [TxEnv],
        spec_id: SpecId,
    ) -> Self {
        Self {
            storage,
            mv_memory,
            chain,
            block_env,
            txs,
            spec_id,
            beneficiary_location_hash: hash_determinisitic(MemoryLocation::Basic(
                block_env.coinbase,
            )),
            reward_policy: chain.get_reward_policy(),
        }
    }

    // Execute a transaction. This can read from memory but cannot modify any state.
    // A successful execution returns:
    //   - A write-set consisting of memory locations and their updated values.
    //   - A read-set consisting of memory locations and their origins.
    //
    // An execution may observe a read dependency on a lower transaction. This happens
    // when the last incarnation of the dependency wrote to a memory location that
    // this transaction reads, but it aborted before the read. In this case, the
    // dependency index is returned via [blocking_tx_idx]. An execution task for this
    // transaction is re-scheduled after the blocking dependency finishes its
    // next incarnation.
    //
    // When a transaction attempts to write a value to a location, the location and
    // value are added to the write set, possibly replacing a pair with a prior value
    // (if it is not the first time the transaction wrote to this location during the
    // execution).
    pub(crate) fn execute(
        &self,
        tx_version: &TxVersion,
    ) -> Result<VmExecutionResult, VmExecutionError> {
        // SAFETY: A correct scheduler would guarantee this index to be inbound.
        let tx = unsafe { self.txs.get_unchecked(tx_version.tx_idx) };
        let from_hash = hash_determinisitic(MemoryLocation::Basic(tx.caller));
        let to_hash = tx
            .transact_to
            .to()
            .map(|to| hash_determinisitic(MemoryLocation::Basic(*to)));

        // Execute
        let mut db = VmDb::new(self, tx_version.tx_idx, tx, from_hash, to_hash)
            .map_err(VmExecutionError::from)?;
        // TODO: Share as much [Evm], [Context], [Handler], etc. among threads as possible
        // as creating them is very expensive.
        let mut evm = build_evm(
            &mut db,
            self.chain,
            self.spec_id,
            self.block_env.clone(),
            Some(tx.clone()),
            false,
        );
        match evm.transact() {
            Ok(result_and_state) => {
                // There are at least three locations most of the time: the sender,
                // the recipient, and the beneficiary accounts.
                let mut write_set = WriteSet::with_capacity(3);
                for (address, account) in &result_and_state.state {
                    if account.is_selfdestructed() {
                        // TODO: Also write [SelfDestructed] to the basic location?
                        // For now we are betting on [code_hash] triggering the sequential
                        // fallback when we read a self-destructed contract.
                        write_set.push((
                            hash_determinisitic(MemoryLocation::CodeHash(*address)),
                            MemoryValue::SelfDestructed,
                        ));
                        continue;
                    }

                    if account.is_touched() {
                        let account_location_hash =
                            hash_determinisitic(MemoryLocation::Basic(*address));
                        let read_account = evm.db().read_accounts.get(&account_location_hash);

                        let has_code = !account.info.is_empty_code_hash();
                        let is_new_code = has_code
                            && read_account.map_or(true, |(_, code_hash)| code_hash.is_none());

                        // Write new account changes
                        if is_new_code
                            || read_account.is_none()
                            || read_account.is_some_and(|(basic, _)| {
                                basic.nonce != account.info.nonce
                                    || basic.balance != account.info.balance
                            })
                        {
                            if evm.db().is_lazy {
                                if account_location_hash == from_hash {
                                    write_set.push((
                                        account_location_hash,
                                        MemoryValue::LazySender(U256::MAX - account.info.balance),
                                    ));
                                } else if Some(account_location_hash) == to_hash {
                                    write_set.push((
                                        account_location_hash,
                                        MemoryValue::LazyRecipient(tx.value),
                                    ));
                                }
                            }
                            // We don't register empty accounts after [SPURIOUS_DRAGON]
                            // as they are cleared. This can only happen via 2 ways:
                            // 1. Self-destruction which is handled by an if above.
                            // 2. Sending 0 ETH to an empty account, which we treat as a
                            // non-write here. A later read would trace back to storage
                            // and return a [None], i.e., [LoadedAsNotExisting]. Without
                            // this check it would write then read a [Some] default
                            // account, which may yield a wrong gas fee, etc.
                            else if !self.chain.is_eip_161_enabled(self.spec_id)
                                || !account.is_empty()
                            {
                                write_set.push((
                                    account_location_hash,
                                    MemoryValue::Basic(AccountBasic {
                                        balance: account.info.balance,
                                        nonce: account.info.nonce,
                                    }),
                                ));
                            }
                        }

                        // Write new contract
                        if is_new_code {
                            write_set.push((
                                hash_determinisitic(MemoryLocation::CodeHash(*address)),
                                MemoryValue::CodeHash(account.info.code_hash),
                            ));
                            self.mv_memory
                                .new_bytecodes
                                .entry(account.info.code_hash)
                                .or_insert_with(|| account.info.code.clone().unwrap());
                        }
                    }

                    // TODO: We should move this changed check to our read set like for account info?
                    for (slot, value) in account.changed_storage_slots() {
                        write_set.push((
                            hash_determinisitic(MemoryLocation::Storage(*address, *slot)),
                            MemoryValue::Storage(value.present_value),
                        ));
                    }
                }

                self.apply_rewards(
                    &mut write_set,
                    tx,
                    U256::from(result_and_state.result.gas_used()),
                    #[cfg(feature = "optimism")]
                    &evm.context.evm,
                )?;

                drop(evm); // release db

                if db.is_lazy {
                    self.mv_memory
                        .add_lazy_addresses([tx.caller, *tx.transact_to.to().unwrap()]);
                }

                let mut flags = if tx_version.tx_idx > 0 && !db.is_lazy {
                    FinishExecFlags::NeedValidation
                } else {
                    FinishExecFlags::empty()
                };

                if self.mv_memory.record(tx_version, db.read_set, write_set) {
                    flags |= FinishExecFlags::WroteNewLocation;
                }

                Ok(VmExecutionResult {
                    execution_result: PevmTxExecutionResult::from_revm(
                        self.chain,
                        self.spec_id,
                        result_and_state,
                    ),
                    flags,
                })
            }
            Err(EVMError::Database(read_error)) => Err(read_error.into()),
            Err(err) => {
                // Optimistically retry in case some previous internal transactions send
                // more fund to the sender but hasn't been executed yet.
                // TODO: Let users define this behaviour through a mode enum or something.
                // Since this retry is safe for syncing canonical blocks but can deadlock
                // on new or faulty blocks. We can skip the transaction for new blocks and
                // error out after a number of tries for the latter.
                if tx_version.tx_idx > 0
                    && matches!(
                        err,
                        EVMError::Transaction(
                            InvalidTransaction::LackOfFundForMaxFee { .. }
                                | InvalidTransaction::NonceTooHigh { .. }
                        )
                    )
                {
                    Err(VmExecutionError::Blocking(tx_version.tx_idx - 1))
                } else {
                    Err(VmExecutionError::ExecutionError(err))
                }
            }
        }
    }

    // Apply rewards (balance increments) to beneficiary accounts, etc.
    fn apply_rewards<#[cfg(feature = "optimism")] DB: Database>(
        &self,
        write_set: &mut WriteSet,
        tx: &TxEnv,
        gas_used: U256,
        #[cfg(feature = "optimism")] evm_context: &EvmContext<DB>,
    ) -> Result<(), VmExecutionError> {
        let mut gas_price = if let Some(priority_fee) = tx.gas_priority_fee {
            std::cmp::min(
                tx.gas_price,
                priority_fee.saturating_add(self.block_env.basefee),
            )
        } else {
            tx.gas_price
        };
        if self.chain.is_eip_1559_enabled(self.spec_id) {
            gas_price = gas_price.saturating_sub(self.block_env.basefee);
        }

        let rewards: SmallVec<[(MemoryLocationHash, U256); 1]> = match self.reward_policy {
            RewardPolicy::Ethereum => {
                smallvec![(
                    self.beneficiary_location_hash,
                    gas_price.saturating_mul(gas_used)
                )]
            }
            #[cfg(feature = "optimism")]
            RewardPolicy::Optimism {
                l1_fee_recipient_location_hash,
                base_fee_vault_location_hash,
            } => {
                let is_deposit = tx.optimism.source_hash.is_some();
                if is_deposit {
                    SmallVec::new()
                } else {
                    // TODO: Better error handling
                    // https://github.com/bluealloy/revm/blob/16e1ecb9a71544d9f205a51a22d81e2658202fde/crates/revm/src/optimism/handler_register.rs#L267
                    let Some(enveloped_tx) = &tx.optimism.enveloped_tx else {
                        panic!("[OPTIMISM] Failed to load enveloped transaction.");
                    };
                    let Some(l1_block_info) = &evm_context.l1_block_info else {
                        panic!("[OPTIMISM] Missing l1_block_info.");
                    };
                    let l1_cost = l1_block_info.calculate_tx_l1_cost(enveloped_tx, self.spec_id);

                    smallvec![
                        (
                            self.beneficiary_location_hash,
                            gas_price.saturating_mul(gas_used)
                        ),
                        (l1_fee_recipient_location_hash, l1_cost),
                        (
                            base_fee_vault_location_hash,
                            self.block_env.basefee.saturating_mul(gas_used),
                        ),
                    ]
                }
            }
        };

        for (recipient, amount) in rewards {
            if let Some((_, value)) = write_set
                .iter_mut()
                .find(|(location, _)| location == &recipient)
            {
                match value {
                    MemoryValue::Basic(basic) => {
                        basic.balance = basic.balance.saturating_add(amount)
                    }
                    MemoryValue::LazySender(subtraction) => {
                        *subtraction = subtraction.saturating_sub(amount)
                    }
                    MemoryValue::LazyRecipient(addition) => {
                        *addition = addition.saturating_add(amount)
                    }
                    _ => return Err(ReadError::InvalidMemoryValueType.into()),
                }
            } else {
                write_set.push((recipient, MemoryValue::LazyRecipient(amount)));
            }
        }

        Ok(())
    }
}

pub(crate) fn build_evm<'a, DB: Database, C: PevmChain>(
    db: DB,
    chain: &C,
    spec_id: SpecId,
    block_env: BlockEnv,
    tx_env: Option<TxEnv>,
    with_reward_beneficiary: bool,
) -> Evm<'a, (), DB> {
    // This is much uglier than the builder interface but can be up to 50% faster!!
    let context = Context {
        evm: EvmContext::new_with_env(
            db,
            Env::boxed(
                CfgEnv::default().with_chain_id(chain.id()),
                block_env,
                tx_env.unwrap_or_default(),
            ),
        ),
        external: (),
    };

    let handler = chain.get_handler(spec_id, with_reward_beneficiary);
    Evm::new(context, handler)
}
