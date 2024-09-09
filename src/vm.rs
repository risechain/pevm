use ahash::{AHashMap, HashMapExt};
use alloy_primitives::TxKind;
use alloy_rpc_types::Receipt;
use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CfgEnv, EVMError, Env, InvalidTransaction,
        ResultAndState,
        SpecId::{self, SPURIOUS_DRAGON},
        TxEnv, B256, KECCAK_EMPTY, U256,
    },
    Context, Database, Evm, EvmContext,
};
use smallvec::{smallvec, SmallVec};
use std::{cmp::Ordering, collections::HashMap};

use crate::{
    chain::{PevmChain, RewardPolicy},
    mv_memory::MvMemory,
    AccountBasic, BuildIdentityHasher, BuildSuffixHasher, EvmAccount, FinishExecFlags, MemoryEntry,
    MemoryLocation, MemoryLocationHash, MemoryValue, ReadError, ReadOrigin, ReadOrigins, ReadSet,
    Storage, TxIdx, TxVersion, WriteSet,
};

/// The execution error from the underlying EVM executor.
// Will there be DB errors outside of read?
pub type ExecutionError = EVMError<ReadError>;

/// Represents the state transitions of the EVM accounts after execution.
/// If the value is [None], it indicates that the account is marked for removal.
/// If the value is [Some(new_state)], it indicates that the account has become [new_state].
type EvmStateTransitions = HashMap<Address, Option<EvmAccount>, BuildSuffixHasher>;

/// Execution result of a transaction
#[derive(Debug, Clone, PartialEq)]
pub struct PevmTxExecutionResult {
    /// Receipt of execution
    // TODO: Consider promoting to [ReceiptEnvelope] if there is high demand
    pub receipt: Receipt,
    /// State that got updated
    pub state: EvmStateTransitions,
}

impl PevmTxExecutionResult {
    /// Construct a Pevm execution result from a raw Revm result.
    /// Note that [cumulative_gas_used] is preset to the gas used of this transaction.
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

// TODO: Rewrite as [Result]
pub(crate) enum VmExecutionResult {
    Retry,
    FallbackToSequential,
    ReadError {
        blocking_tx_idx: TxIdx,
    },
    ExecutionError(ExecutionError),
    Ok {
        execution_result: PevmTxExecutionResult,
        flags: FinishExecFlags,
    },
}

#[derive(Debug, PartialEq)]
enum LazyStrategy {
    None,
    RawTransfer,
    ERC20Transfer { amount: U256 },
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
    lazy_strategy: LazyStrategy,
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
            lazy_strategy: LazyStrategy::None,
            // Unless it is a raw transfer that is lazy updated, we'll
            // read at least from the sender and recipient accounts.
            read_set: ReadSet::with_capacity(2),
            read_accounts: HashMap::with_capacity_and_hasher(2, BuildIdentityHasher::default()),
        };
        // TODO: Only lazy update in block syncing mode, not for block
        // building.
        if let TxKind::Call(to) = tx.transact_to {
            db.to_code_hash = db.get_code_hash(to)?;
            if db.to_code_hash.is_none() {
                // We only lazy update raw transfers that already have the sender
                // or recipient in [MvMemory] since sequentially evaluating memory
                // locations with only one entry is much costlier than fully
                // evaluating it concurrently.
                db.lazy_strategy = if vm.mv_memory.data.contains_key(&from_hash)
                    || vm.mv_memory.data.contains_key(&to_hash.unwrap())
                {
                    LazyStrategy::RawTransfer
                } else {
                    LazyStrategy::None
                }
            } else if vm.chain.is_erc20_transfer(&vm.txs[tx_idx]) {
                // Similarly, we don't want to lazy update for ERC20 transfers
                // if the contract address is not being called too many times.
                db.lazy_strategy =
                    if vm.recipient_frequency.get(&to).copied().unwrap_or_default() >= 4 {
                        let input = &vm.txs[tx_idx].data;
                        LazyStrategy::ERC20Transfer {
                            amount: U256::from_be_slice(&input[36..68]),
                        }
                    } else {
                        LazyStrategy::None
                    }
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
        self.vm.hash_basic(*address)
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
        let location_hash = self.vm.hasher.hash_one(MemoryLocation::CodeHash(address));
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
        if self.lazy_strategy == LazyStrategy::RawTransfer {
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
                            return Err(ReadError::BlockingIndex(*blocking_idx))
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
                                        balance_addition += addition;
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
                                        balance_addition += subtraction;
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
                Ok(None) => {
                    if balance_addition > U256::ZERO {
                        Some(AccountBasic::default())
                    } else {
                        None
                    }
                }
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
                if self.tx_idx > 0 {
                    // TODO: Better retry strategy -- immediately, to the
                    // closest sender tx, to the missing sender tx, etc.
                    return Err(ReadError::BlockingIndex(self.tx_idx - 1));
                } else {
                    return Err(ReadError::InvalidNonce(self.tx_idx));
                }
            }

            // Fully evaluate the account and register it to read cache
            // to later check if they have changed (been written to).
            if positive_addition {
                account.balance += balance_addition;
            } else {
                account.balance -= balance_addition;
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
                        Ok(code) => code.map(Bytecode::from),
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
        self.vm
            .storage
            .code_by_hash(&code_hash)
            .map(|code| code.map(Bytecode::from).unwrap_or_default())
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }

    fn has_storage(&mut self, address: Address) -> Result<bool, Self::Error> {
        self.vm
            .storage
            .has_storage(&address)
            .map_err(|err| ReadError::StorageError(err.to_string()))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        if let LazyStrategy::ERC20Transfer { amount } = self.lazy_strategy {
            if TxKind::Call(address) == self.tx.transact_to {
                // Using the storage value is the best guess to match the zeroness.
                // The zeroness of the storage slot (before and after tx) affects the gas used.
                let storage_value = self
                    .vm
                    .storage
                    .storage(&address, &index)
                    .map_err(|err| ReadError::StorageError(err.to_string()))?;

                // If the storage_value is big enough, we don't need to care
                // whether the storage slot is sender or recipient.
                if storage_value >= amount {
                    return Ok(storage_value);
                }

                // If the storage_value is zero, it should be a recipient.
                // We should not touch.
                if storage_value.is_zero() {
                    return Ok(storage_value);
                }

                // Otherwise, let's assume that this is a recipient.
                // In this case, an arbitrarily large value is okay.
                return Ok(U256::from(1).wrapping_shl(255));
            }
        }

        let location_hash = self
            .vm
            .hasher
            .hash_one(MemoryLocation::Storage(address, index));

        let read_origins = self.read_set.entry(location_hash).or_default();
        let has_prev_origins = !read_origins.is_empty();
        // We accumulate new origins to either:
        // - match with the previous origins to check consistency
        // - register origins on the first read
        let mut new_origins = SmallVec::new();
        let mut accumulated_addition = U256::ZERO;
        let mut accumulated_subtraction = U256::ZERO;

        // Try reading from multi-version data
        if self.tx_idx > 0 {
            if let Some(written_transactions) = self.vm.mv_memory.data.get(&location_hash) {
                let mut iter = written_transactions.range(..self.tx_idx);
                loop {
                    match iter.next_back() {
                        Some((blocking_idx, MemoryEntry::Estimate)) => {
                            return Err(ReadError::BlockingIndex(*blocking_idx))
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
                                MemoryValue::Storage(storage_value) => {
                                    if !has_prev_origins {
                                        *read_origins = new_origins;
                                    }
                                    return Ok(storage_value + accumulated_addition
                                        - accumulated_subtraction);
                                }
                                MemoryValue::ERC20TransferRecipient { amount, .. } => {
                                    accumulated_addition += amount;
                                }
                                MemoryValue::ERC20TransferSender { amount, .. } => {
                                    accumulated_subtraction += amount;
                                }
                                _ => return Err(ReadError::InvalidMemoryValueType),
                            }
                        }
                        None => {
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
                            let value_from_storage = self
                                .vm
                                .storage
                                .storage(&address, &index)
                                .map_err(|err| ReadError::StorageError(err.to_string()))?;
                            // Populate read origins on the first read.
                            // Otherwise [read_origins] matches [new_origins] already.
                            if !has_prev_origins {
                                *read_origins = new_origins;
                            }
                            return Ok(
                                value_from_storage + accumulated_addition - accumulated_subtraction
                            );
                        }
                    }
                }
            }
        }

        // Fall back to storage
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
        if !has_prev_origins {
            *read_origins = new_origins;
        }
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
    hasher: &'a ahash::RandomState,
    storage: &'a S,
    mv_memory: &'a MvMemory,
    chain: &'a C,
    block_env: &'a BlockEnv,
    txs: &'a [TxEnv],
    spec_id: SpecId,
    beneficiary_location_hash: MemoryLocationHash,
    reward_policy: RewardPolicy,
    recipient_frequency: AHashMap<Address, usize>,
}

impl<'a, S: Storage, C: PevmChain> Vm<'a, S, C> {
    pub(crate) fn new(
        hasher: &'a ahash::RandomState,
        storage: &'a S,
        mv_memory: &'a MvMemory,
        chain: &'a C,
        block_env: &'a BlockEnv,
        txs: &'a [TxEnv],
        spec_id: SpecId,
    ) -> Self {
        let mut recipient_frequency = AHashMap::new();
        for tx in txs.iter() {
            if let TxKind::Call(address) = tx.transact_to {
                *recipient_frequency.entry(address).or_default() += 1;
            }
        }

        Self {
            hasher,
            storage,
            mv_memory,
            chain,
            block_env,
            txs,
            spec_id,
            beneficiary_location_hash: hasher.hash_one(MemoryLocation::Basic(block_env.coinbase)),
            reward_policy: chain.get_reward_policy(hasher),
            recipient_frequency,
        }
    }

    #[inline(always)]
    fn hash_basic(&self, address: Address) -> MemoryLocationHash {
        self.hasher.hash_one(MemoryLocation::Basic(address))
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
    pub(crate) fn execute(&self, tx_version: &TxVersion) -> VmExecutionResult {
        // SAFETY: A correct scheduler would guarantee this index to be inbound.
        let tx = unsafe { self.txs.get_unchecked(tx_version.tx_idx) };
        let from_hash = self.hash_basic(tx.caller);
        let to_hash = tx.transact_to.to().map(|to| self.hash_basic(*to));

        // Execute
        let mut db = match VmDb::new(self, tx_version.tx_idx, tx, from_hash, to_hash) {
            Ok(db) => db,
            // TODO: Handle different errors differently
            Err(_) => return VmExecutionResult::FallbackToSequential,
        };
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
                let mut lazy_locations: SmallVec<[MemoryLocation; 2]> = SmallVec::new();

                for (address, account) in result_and_state.state.iter() {
                    if account.is_selfdestructed() {
                        // TODO: Also write [SelfDestructed] to the basic location?
                        // For now we are betting on [code_hash] triggering the sequential
                        // fallback when we read a self-destructed contract.
                        write_set.push((
                            self.hasher.hash_one(MemoryLocation::CodeHash(*address)),
                            MemoryValue::SelfDestructed,
                        ));
                        continue;
                    }

                    if account.is_touched() {
                        let account_location_hash = self.hash_basic(*address);
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
                            match evm.db().lazy_strategy {
                                LazyStrategy::RawTransfer => {
                                    if account_location_hash == from_hash {
                                        lazy_locations.push(MemoryLocation::Basic(*address));
                                        write_set.push((
                                            account_location_hash,
                                            MemoryValue::LazySender(
                                                U256::MAX - account.info.balance,
                                            ),
                                        ));
                                    } else if Some(account_location_hash) == to_hash {
                                        lazy_locations.push(MemoryLocation::Basic(*address));
                                        write_set.push((
                                            account_location_hash,
                                            MemoryValue::LazyRecipient(tx.value),
                                        ));
                                    }
                                }
                                LazyStrategy::ERC20Transfer { .. } | LazyStrategy::None => {
                                    // We don't register empty accounts after [SPURIOUS_DRAGON]
                                    // as they are cleared. This can only happen via 2 ways:
                                    // 1. Self-destruction which is handled by an if above.
                                    // 2. Sending 0 ETH to an empty account, which we treat as a
                                    // non-write here. A later read would trace back to storage
                                    // and return a [None], i.e., [LoadedAsNotExisting]. Without
                                    // this check it would write then read a [Some] default
                                    // account, which may yield a wrong gas fee, etc.
                                    if !self.spec_id.is_enabled_in(SPURIOUS_DRAGON)
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
                            }
                        }

                        // Write new contract
                        if is_new_code {
                            write_set.push((
                                self.hasher.hash_one(MemoryLocation::CodeHash(*address)),
                                MemoryValue::CodeHash(account.info.code_hash),
                            ));
                            self.mv_memory
                                .new_bytecodes
                                .entry(account.info.code_hash)
                                .or_insert_with(|| account.info.code.clone().unwrap());
                        }
                    }

                    // TODO: We should move this changed check to our read set like for account info?
                    for (&slot, value) in account.changed_storage_slots() {
                        let memory_location = MemoryLocation::Storage(*address, slot);
                        let memory_location_hash = self.hasher.hash_one(&memory_location);
                        match evm.db().lazy_strategy {
                            LazyStrategy::None => {
                                write_set.push((
                                    memory_location_hash,
                                    MemoryValue::Storage(value.present_value),
                                ));
                            }
                            LazyStrategy::ERC20Transfer { .. } => {
                                lazy_locations.push(memory_location);
                                match Ord::cmp(&value.present_value, &value.original_value) {
                                    Ordering::Less => {
                                        write_set.push((
                                            memory_location_hash,
                                            MemoryValue::ERC20TransferSender {
                                                amount: value.original_value - value.present_value,
                                                zeroness_before: value.original_value.is_zero(),
                                                zeroness_after: value.present_value.is_zero(),
                                            },
                                        ));
                                    }
                                    Ordering::Greater => {
                                        write_set.push((
                                            memory_location_hash,
                                            MemoryValue::ERC20TransferRecipient {
                                                amount: value.present_value - value.original_value,
                                                zeroness_before: value.original_value.is_zero(),
                                                zeroness_after: value.present_value.is_zero(),
                                            },
                                        ));
                                    }
                                    Ordering::Equal => {
                                        // Unreachable because we are iterating `account.CHANGED_storage_slots()`.
                                        unreachable!();
                                    }
                                }
                            }
                            LazyStrategy::RawTransfer => unreachable!(),
                        }
                    }
                }

                if !lazy_locations.is_empty() {
                    let mut guard = self.mv_memory.lazy_locations.lock().unwrap();
                    guard.extend(lazy_locations);
                    drop(guard)
                }

                self.apply_rewards(
                    &mut write_set,
                    tx,
                    U256::from(result_and_state.result.gas_used()),
                );

                drop(evm); // release db

                let mut flags =
                    if tx_version.tx_idx > 0 && matches!(db.lazy_strategy, LazyStrategy::None) {
                        FinishExecFlags::NeedValidation
                    } else {
                        FinishExecFlags::empty()
                    };

                if self.mv_memory.record(tx_version, db.read_set, write_set) {
                    flags |= FinishExecFlags::WroteNewLocation;
                }

                VmExecutionResult::Ok {
                    execution_result: PevmTxExecutionResult::from_revm(
                        self.spec_id,
                        result_and_state,
                    ),
                    flags,
                }
            }
            Err(EVMError::Database(ReadError::InconsistentRead)) => VmExecutionResult::Retry,
            Err(EVMError::Database(ReadError::SelfDestructedAccount)) => {
                VmExecutionResult::FallbackToSequential
            }
            Err(EVMError::Database(ReadError::BlockingIndex(blocking_tx_idx))) => {
                VmExecutionResult::ReadError { blocking_tx_idx }
            }
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
                        EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee { .. })
                            | EVMError::Transaction(InvalidTransaction::NonceTooHigh { .. })
                    )
                {
                    VmExecutionResult::ReadError {
                        blocking_tx_idx: tx_version.tx_idx - 1,
                    }
                } else {
                    VmExecutionResult::ExecutionError(err)
                }
            }
        }
    }

    // Apply rewards (balance increments) to beneficiary accounts, etc.
    fn apply_rewards(&self, write_set: &mut WriteSet, tx: &TxEnv, gas_used: U256) {
        let rewards: SmallVec<[(MemoryLocationHash, U256); 1]> = match self.reward_policy {
            RewardPolicy::Ethereum => {
                let mut gas_price = if let Some(priority_fee) = tx.gas_priority_fee {
                    std::cmp::min(tx.gas_price, priority_fee + self.block_env.basefee)
                } else {
                    tx.gas_price
                };
                if self.spec_id.is_enabled_in(SpecId::LONDON) {
                    gas_price = gas_price.saturating_sub(self.block_env.basefee);
                }
                smallvec![(self.beneficiary_location_hash, gas_price * gas_used)]
            }
        };

        for (recipient, amount) in rewards {
            if let Some((_, value)) = write_set
                .iter_mut()
                .find(|(location, _)| location == &recipient)
            {
                match value {
                    MemoryValue::Basic(basic) => basic.balance += amount,
                    MemoryValue::LazySender(addition) => *addition -= amount,
                    MemoryValue::LazyRecipient(addition) => *addition += amount,
                    _ => unreachable!(), // TODO: Better error handling
                }
            } else {
                write_set.push((recipient, MemoryValue::LazyRecipient(amount)));
            }
        }
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
