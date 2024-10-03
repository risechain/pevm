use std::{
    fmt::Debug,
    num::NonZeroUsize,
    sync::{mpsc, Mutex, OnceLock},
    thread,
};

use ahash::AHashMap;
use alloy_primitives::U256;
use alloy_rpc_types::{Block, BlockTransactions};
use revm::{
    db::CacheDB,
    primitives::{BlockEnv, SpecId, TxEnv},
    DatabaseCommit,
};

use crate::{
    chain::PevmChain,
    compat::get_block_env,
    mv_memory::MvMemory,
    scheduler::Scheduler,
    storage::StorageWrapper,
    vm::{
        build_evm, ExecutionError, PevmTxExecutionResult, Vm, VmExecutionError, VmExecutionResult,
    },
    EvmAccount, MemoryEntry, MemoryLocation, MemoryValue, Storage, Task, TxVersion,
};

/// Errors when executing a block with pevm.
#[derive(Debug, Clone, PartialEq)]
pub enum PevmError<C: PevmChain> {
    /// Cannot derive the chain spec from the block header.
    BlockSpecError(C::BlockSpecError),
    /// Transactions lack information for execution.
    MissingTransactionData,
    /// Invalid input transaction.
    InvalidTransaction(C::TransactionParsingError),
    /// Storage error.
    // TODO: More concrete types than just an arbitrary string.
    StorageError(String),
    /// EVM execution error.
    // TODO: More concrete types than just an arbitrary string.
    ExecutionError(String),
    /// Impractical errors that should be unreachable.
    /// The library has bugs if this is yielded.
    UnreachableError,
}

/// Execution result of a block
pub type PevmResult<C> = Result<Vec<PevmTxExecutionResult>, PevmError<C>>;

#[derive(Debug)]
enum AbortReason {
    FallbackToSequential,
    ExecutionError(ExecutionError),
}

// TODO: Better implementation
#[derive(Debug)]
struct AsyncDropper<T> {
    sender: mpsc::Sender<T>,
    _handle: thread::JoinHandle<()>,
}

impl<T: Send + 'static> Default for AsyncDropper<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            _handle: std::thread::spawn(move || receiver.into_iter().for_each(drop)),
        }
    }
}

impl<T> AsyncDropper<T> {
    fn drop(&self, t: T) {
        // TODO: Better error handling
        self.sender.send(t).unwrap();
    }
}

// TODO: Port more recyclable resources into here.
#[derive(Debug, Default)]
/// The main pevm struct that executes blocks.
pub struct Pevm {
    hasher: ahash::RandomState,
    execution_results: Vec<Mutex<Option<PevmTxExecutionResult>>>,
    abort_reason: OnceLock<AbortReason>,
    dropper: AsyncDropper<(MvMemory, Scheduler, Vec<TxEnv>)>,
}

impl Pevm {
    /// Execute an Alloy block, which is becoming the "standard" format in Rust.
    /// TODO: Better error handling.
    pub fn execute<S: Storage + Send + Sync, C: PevmChain + Send + Sync>(
        &mut self,
        storage: &S,
        chain: &C,
        block: Block<C::Transaction>,
        concurrency_level: NonZeroUsize,
        force_sequential: bool,
    ) -> PevmResult<C> {
        let spec_id = chain
            .get_block_spec(&block.header)
            .map_err(PevmError::BlockSpecError)?;
        let block_env = get_block_env(&block.header);
        let tx_envs = match block.transactions {
            BlockTransactions::Full(txs) => txs
                .into_iter()
                .map(|tx| chain.get_tx_env(tx))
                .collect::<Result<Vec<TxEnv>, _>>()
                .map_err(PevmError::InvalidTransaction)?,
            _ => return Err(PevmError::MissingTransactionData),
        };
        // TODO: Continue to fine tune this condition.
        if force_sequential
            || tx_envs.len() < concurrency_level.into()
            || block.header.gas_used < 4_000_000
        {
            execute_revm_sequential(storage, chain, spec_id, block_env, tx_envs)
        } else {
            self.execute_revm_parallel(
                storage,
                chain,
                spec_id,
                block_env,
                tx_envs,
                concurrency_level,
            )
        }
    }

    /// Execute an REVM block.
    // Ideally everyone would go through the [Alloy] interface. This one is currently
    // useful for testing, and for users that are heavily tied to Revm like Reth.
    pub fn execute_revm_parallel<S: Storage + Send + Sync, C: PevmChain + Send + Sync>(
        &mut self,
        storage: &S,
        chain: &C,
        spec_id: SpecId,
        block_env: BlockEnv,
        txs: Vec<TxEnv>,
        concurrency_level: NonZeroUsize,
    ) -> PevmResult<C> {
        if txs.is_empty() {
            return Ok(Vec::new());
        }

        let block_size = txs.len();
        let scheduler = Scheduler::new(block_size);

        let mv_memory = chain.build_mv_memory(&self.hasher, &block_env, &txs);
        let vm = Vm::new(
            &self.hasher,
            storage,
            &mv_memory,
            chain,
            &block_env,
            &txs,
            spec_id,
        );

        if block_size > self.execution_results.len() {
            let additional = block_size.saturating_sub(self.execution_results.len());
            self.execution_results.reserve(additional);
            for _ in 0..additional {
                self.execution_results.push(Mutex::new(None));
            }
        }

        // TODO: Better thread handling
        thread::scope(|scope| {
            for _ in 0..concurrency_level.into() {
                scope.spawn(|| {
                    let mut task = scheduler.next_task();
                    while task.is_some() {
                        task = match task.unwrap() {
                            Task::Execution(tx_version) => {
                                self.try_execute(&vm, &scheduler, tx_version)
                            }
                            Task::Validation(tx_version) => {
                                try_validate(&mv_memory, &scheduler, &tx_version)
                            }
                        };

                        // TODO: Have different functions or an enum for the caller to choose
                        // the handling behaviour when a transaction's EVM execution fails.
                        // Parallel block builders would like to exclude such transaction,
                        // verifiers may want to exit early to save CPU cycles, while testers
                        // may want to collect all execution results. We are exiting early as
                        // the default behaviour for now.
                        if self.abort_reason.get().is_some() {
                            break;
                        }

                        if task.is_none() {
                            task = scheduler.next_task();
                        }
                    }
                });
            }
        });

        if let Some(abort_reason) = self.abort_reason.take() {
            match abort_reason {
                AbortReason::FallbackToSequential => {
                    self.dropper.drop((mv_memory, scheduler, Vec::new()));
                    return execute_revm_sequential(storage, chain, spec_id, block_env, txs);
                }
                AbortReason::ExecutionError(err) => {
                    self.dropper.drop((mv_memory, scheduler, txs));
                    return Err(PevmError::ExecutionError(format!("{err:?}")));
                }
            }
        }

        let mut fully_evaluated_results = Vec::with_capacity(block_size);
        let mut cumulative_gas_used: u128 = 0;
        for i in 0..block_size {
            let mut execution_result = index_mutex!(self.execution_results, i).take().unwrap();
            cumulative_gas_used = cumulative_gas_used.saturating_add(execution_result.receipt.cumulative_gas_used);
            execution_result.receipt.cumulative_gas_used = cumulative_gas_used;
            fully_evaluated_results.push(execution_result);
        }

        // We fully evaluate (the balance and nonce of) the beneficiary account
        // and raw transfer recipients that may have been atomically updated.
        for address in mv_memory.consume_lazy_addresses() {
            let location_hash = self.hasher.hash_one(MemoryLocation::Basic(address));
            if let Some(write_history) = mv_memory.data.get(&location_hash) {
                let mut balance = U256::ZERO;
                let mut nonce = 0;
                // Read from storage if the first multi-version entry is not an absolute value.
                if !matches!(
                    write_history.first_key_value(),
                    Some((_, MemoryEntry::Data(_, MemoryValue::Basic(_))))
                ) {
                    if let Ok(Some(account)) = storage.basic(&address) {
                        balance = account.balance;
                        nonce = account.nonce;
                    }
                }
                // Accounts that take implicit writes like the beneficiary account can be contract!
                let code_hash = match storage.code_hash(&address) {
                    Ok(code_hash) => code_hash,
                    Err(err) => return Err(PevmError::StorageError(err.to_string())),
                };
                let code = if let Some(code_hash) = &code_hash {
                    match storage.code_by_hash(code_hash) {
                        Ok(code) => code,
                        Err(err) => return Err(PevmError::StorageError(err.to_string())),
                    }
                } else {
                    None
                };

                for (tx_idx, memory_entry) in write_history.iter() {
                    let tx = unsafe { txs.get_unchecked(*tx_idx) };
                    match memory_entry {
                        MemoryEntry::Data(_, MemoryValue::Basic(info)) => {
                            // We fall back to sequential execution when reading a self-destructed account,
                            // so an empty account here would be a bug
                            debug_assert!(!(info.balance.is_zero() && info.nonce == 0));
                            balance = info.balance;
                            nonce = info.nonce;
                        }
                        MemoryEntry::Data(_, MemoryValue::LazyRecipient(addition)) => {
                            balance = balance.saturating_add(*addition);
                        }
                        MemoryEntry::Data(_, MemoryValue::LazySender(addition)) => {
                            // We must re-do extra sender balance checks as we mock
                            // the max value in [Vm] during execution. Ideally we
                            // can turn off these redundant checks in revm.
                            // TODO: Guard against overflows & underflows
                            // Ideally we would share these calculations with revm
                            // (using their utility functions).
                            let mut max_fee = (U256::from(tx.gas_limit) * tx.gas_price).saturating_add(tx.value);
                            if let Some(blob_fee) = tx.max_fee_per_blob_gas {
                                max_fee =max_fee.saturating_add(
                                    U256::from(tx.get_total_blob_gas()) * U256::from(blob_fee));
                            }
                            if balance < max_fee {
                                return Err(PevmError::ExecutionError(
                                    "Transaction(LackOfFundForMaxFee)".to_string(),
                                ));
                            }
                            balance = balance.saturating_sub(*addition);
                            // End of overflow TODO

                            nonce += 1;
                        }
                        // TODO: Better error handling
                        _ => unreachable!(),
                    }
                    // Assert that evaluated nonce is correct when address is caller.
                    debug_assert!(
                        tx.caller != address || tx.nonce.map_or(true, |n| n + 1 == nonce)
                    );

                    // SAFETY: The multi-version data structure should not leak an index over block size.
                    let tx_result = unsafe { fully_evaluated_results.get_unchecked_mut(*tx_idx) };
                    let account = tx_result.state.entry(address).or_default();
                    // TODO: Deduplicate this logic with [PevmTxExecutionResult::from_revm]
                    if chain.is_eip_161_enabled(spec_id)
                        && code_hash.is_none()
                        && nonce == 0
                        && balance == U256::ZERO
                    {
                        *account = None;
                    } else if let Some(account) = account {
                        // Explicit write: only overwrite the account info in case there are storage changes
                        // Code cannot change midblock here as we're falling back to sequential execution
                        // on reading a self-destructed contract.
                        account.balance = balance;
                        account.nonce = nonce;
                    } else {
                        // Implicit write: e.g. gas payments to the beneficiary account,
                        // which doesn't have explicit writes in [tx_result.state]
                        *account = Some(EvmAccount {
                            balance,
                            nonce,
                            code_hash,
                            code: code.clone(),
                            storage: AHashMap::default(),
                        });
                    }
                }
            }
        }

        self.dropper.drop((mv_memory, scheduler, txs));

        Ok(fully_evaluated_results)
    }

    fn try_execute<S: Storage, C: PevmChain>(
        &self,
        vm: &Vm<S, C>,
        scheduler: &Scheduler,
        tx_version: TxVersion,
    ) -> Option<Task> {
        loop {
            return match vm.execute(&tx_version) {
                Err(VmExecutionError::Retry) => {
                    if self.abort_reason.get().is_none() {
                        continue;
                    }
                    None
                }
                Err(VmExecutionError::FallbackToSequential) => {
                    scheduler.abort();
                    self.abort_reason
                        .get_or_init(|| AbortReason::FallbackToSequential);
                    None
                }
                Err(VmExecutionError::Blocking(blocking_tx_idx )) => {
                    if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx)
                        && self.abort_reason.get().is_none()
                    {
                        // Retry the execution immediately if the blocking transaction was
                        // re-executed by the time we can add it as a dependency.
                        continue;
                    }
                    None
                }
                Err(VmExecutionError::ExecutionError(err)) => {
                    scheduler.abort();
                    self.abort_reason
                        .get_or_init(|| AbortReason::ExecutionError(err));
                    None
                }
                Ok(VmExecutionResult {
                    execution_result,
                    flags,
                }) => {
                    *index_mutex!(self.execution_results, tx_version.tx_idx) =
                        Some(execution_result);
                    scheduler.finish_execution(tx_version, flags)
                }
            };
        }
    }
}

fn try_validate(
    mv_memory: &MvMemory,
    scheduler: &Scheduler,
    tx_version: &TxVersion,
) -> Option<Task> {
    let read_set_valid = mv_memory.validate_read_locations(tx_version.tx_idx);
    let aborted = !read_set_valid && scheduler.try_validation_abort(tx_version);
    if aborted {
        mv_memory.convert_writes_to_estimates(tx_version.tx_idx);
    }
    scheduler.finish_validation(tx_version, aborted)
}

/// Execute REVM transactions sequentially.
// Useful for falling back for (small) blocks with many dependencies.
// TODO: Use this for a long chain of sequential transactions even in parallel mode.
pub fn execute_revm_sequential<S: Storage, C: PevmChain>(
    storage: &S,
    chain: &C,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) -> PevmResult<C> {
    let mut db = CacheDB::new(StorageWrapper(storage));
    let mut evm = build_evm(&mut db, chain, spec_id, block_env, None, true);
    let mut results = Vec::with_capacity(txs.len());
    let mut cumulative_gas_used: u128 = 0;
    for tx in txs {
        *evm.tx_mut() = tx;
        match evm.transact() {
            Ok(result_and_state) => {
                evm.db_mut().commit(result_and_state.state.clone());

                let mut execution_result =
                    PevmTxExecutionResult::from_revm(chain, spec_id, result_and_state);

                cumulative_gas_used = cumulative_gas_used.saturating_add(execution_result.receipt.cumulative_gas_used);
                execution_result.receipt.cumulative_gas_used = cumulative_gas_used;

                results.push(execution_result);
            }
            Err(err) => return Err(PevmError::ExecutionError(err.to_string())),
        }
    }
    Ok(results)
}
