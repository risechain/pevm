use std::{
    fmt::Debug,
    num::NonZeroUsize,
    sync::{Mutex, OnceLock},
    thread,
};

use ahash::AHashMap;
use alloy_primitives::U256;
use alloy_rpc_types::{Block, BlockTransactions};
use defer_drop::DeferDrop;
use revm::{
    db::CacheDB,
    primitives::{
        BlockEnv,
        SpecId::{self, SPURIOUS_DRAGON},
        TxEnv,
    },
    DatabaseCommit,
};

use crate::{
    chain::PevmChain,
    mv_memory::MvMemory,
    primitives::{get_block_env, get_tx_env, TransactionParsingError},
    scheduler::Scheduler,
    storage::StorageWrapper,
    vm::{build_evm, ExecutionError, PevmTxExecutionResult, Vm, VmExecutionResult},
    AccountBasic, EvmAccount, MemoryEntry, MemoryLocation, MemoryValue, Storage, Task, TxVersion,
};

/// Errors when executing a block with PEVM.
#[derive(Debug, Clone, PartialEq)]
pub enum PevmError<C: PevmChain> {
    /// Cannot derive the chain spec from the block header.
    BlockSpecError(C::BlockSpecError),
    /// Block header lacks information for execution.
    MissingHeaderData,
    /// Transactions lack information for execution.
    MissingTransactionData,
    /// Invalid input transaction.
    InvalidTransaction(TransactionParsingError<C>),
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

enum AbortReason {
    FallbackToSequential,
    ExecutionError(ExecutionError),
}

// TODO: Add a [Pevm] struct for long-lasting use to minimize
// (de)allocations between runs.

/// Execute an Alloy block, which is becoming the "standard" format in Rust.
/// TODO: Better error handling.
pub fn execute<S: Storage + Send + Sync, C: PevmChain + Send + Sync>(
    storage: &S,
    chain: &C,
    block: Block,
    concurrency_level: NonZeroUsize,
    force_sequential: bool,
) -> PevmResult<C> {
    let spec_id = chain
        .get_block_spec(&block.header)
        .map_err(PevmError::BlockSpecError)?;
    let Some(block_env) = get_block_env(&block.header) else {
        return Err(PevmError::MissingHeaderData);
    };
    let tx_envs = match block.transactions {
        BlockTransactions::Full(txs) => txs
            .into_iter()
            .map(|tx| get_tx_env(chain, tx))
            .collect::<Result<Vec<TxEnv>, TransactionParsingError<_>>>()
            .map_err(PevmError::InvalidTransaction)?,
        _ => return Err(PevmError::MissingTransactionData),
    };
    // TODO: Continue to fine tune this condition.
    if force_sequential || tx_envs.len() < 4 || block.header.gas_used < 2_000_000 {
        execute_revm_sequential(storage, chain, spec_id, block_env, tx_envs)
    } else {
        execute_revm_parallel(
            storage,
            chain,
            spec_id,
            block_env,
            tx_envs,
            concurrency_level,
        )
    }
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
    let mut evm = build_evm(&mut db, chain, spec_id, block_env, true);
    let mut results = Vec::with_capacity(txs.len());
    let mut cumulative_gas_used: u128 = 0;
    for tx in txs {
        *evm.tx_mut() = tx;
        match evm.transact() {
            Ok(result_and_state) => {
                evm.db_mut().commit(result_and_state.state.clone());

                let mut execution_result =
                    PevmTxExecutionResult::from_revm(spec_id, result_and_state);

                cumulative_gas_used += execution_result.receipt.cumulative_gas_used;
                execution_result.receipt.cumulative_gas_used = cumulative_gas_used;

                results.push(execution_result);
            }
            Err(err) => return Err(PevmError::ExecutionError(err.to_string())),
        }
    }
    Ok(results)
}

/// Execute an REVM block.
// Ideally everyone would go through the [Alloy] interface. This one is currently
// useful for testing, and for users that are heavily tied to Revm like Reth.
pub fn execute_revm_parallel<S: Storage + Send + Sync, C: PevmChain + Send + Sync>(
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

    // Preprocess locations
    let block_size = txs.len();
    let hasher = ahash::RandomState::new();
    // Initialize the remaining core components
    // TODO: Provide more explicit garbage collecting configs for users over random background
    // threads like this. For instance, to have a dedicated thread (pool) for cleanup.
    let mv_memory = DeferDrop::new(chain.build_mv_memory(&hasher, &block_env, &txs));
    let txs = DeferDrop::new(txs);
    let vm = Vm::new(
        &hasher, storage, &mv_memory, &block_env, &txs, chain, spec_id,
    );
    let scheduler = DeferDrop::new(Scheduler::new(block_size));

    let mut abort_reason = OnceLock::new();
    let execution_results: Vec<_> = (0..block_size).map(|_| Mutex::new(None)).collect();

    // TODO: Better thread handling
    thread::scope(|scope| {
        for _ in 0..concurrency_level.into() {
            scope.spawn(|| {
                let mut task = scheduler.next_task();
                while task.is_some() {
                    task = match task.unwrap() {
                        Task::Execution(tx_version) => try_execute(
                            &mv_memory,
                            &vm,
                            &scheduler,
                            &abort_reason,
                            &execution_results,
                            tx_version,
                        ),
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
                    if abort_reason.get().is_some() {
                        break;
                    }

                    if task.is_none() {
                        task = scheduler.next_task();
                    }
                }
            });
        }
    });

    if let Some(abort_reason) = abort_reason.take() {
        match abort_reason {
            AbortReason::FallbackToSequential => {
                return execute_revm_sequential(
                    storage,
                    chain,
                    spec_id,
                    block_env,
                    DeferDrop::into_inner(txs),
                )
            }
            AbortReason::ExecutionError(err) => {
                return Err(PevmError::ExecutionError(format!("{err:?}")))
            }
        }
    }

    let mut fully_evaluated_results = Vec::with_capacity(block_size);
    let mut cumulative_gas_used: u128 = 0;
    for mutex in execution_results {
        let mut execution_result = mutex.into_inner().unwrap().unwrap();
        cumulative_gas_used += execution_result.receipt.cumulative_gas_used;
        execution_result.receipt.cumulative_gas_used = cumulative_gas_used;
        fully_evaluated_results.push(execution_result);
    }

    // We fully evaluate (the balance and nonce of) the beneficiary account
    // and raw transfer recipients that may have been atomically updated.
    for address in mv_memory.consume_lazy_addresses() {
        let location_hash = hasher.hash_one(MemoryLocation::Basic(address));
        if let Some(write_history) = mv_memory.consume_location(&location_hash) {
            // TODO: We don't need to read from storage if the first entry is a fully evaluated account.
            let mut current_account = match storage.basic(&address) {
                Ok(Some(account)) => account,
                _ => AccountBasic::default(),
            };
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

            // TODO: Assert that the evaluated nonce matches the tx's.
            for (tx_idx, memory_entry) in write_history {
                match memory_entry {
                    MemoryEntry::Data(_, MemoryValue::Basic(info)) => {
                        if let Some(info) = info {
                            current_account.balance = info.balance;
                            current_account.nonce = info.nonce;
                        }
                        // TODO: Assert that there must be no self-destructed
                        // accounts here.
                    }
                    MemoryEntry::Data(_, MemoryValue::LazyRecipient(addition)) => {
                        current_account.balance += addition;
                    }
                    MemoryEntry::Data(_, MemoryValue::LazySender(addition)) => {
                        // We must re-do extra sender balance checks as we mock
                        // the max value in [Vm] during execution. Ideally we
                        // can turn off these redundant checks in revm.
                        // TODO: Guard against overflows & underflows
                        // Ideally we would share these calculations with revm
                        // (using their utility functions).
                        let tx = &unsafe { txs.get_unchecked(tx_idx) };
                        let mut max_fee = U256::from(tx.gas_limit) * tx.gas_price + tx.value;
                        if let Some(blob_fee) = tx.max_fee_per_blob_gas {
                            max_fee += U256::from(tx.get_total_blob_gas()) * U256::from(blob_fee);
                        }
                        if current_account.balance < max_fee {
                            return Err(PevmError::ExecutionError(
                                "Transaction(LackOfFundForMaxFee)".to_string(),
                            ));
                        }
                        current_account.balance -= addition;
                        // End of overflow TODO

                        current_account.nonce += 1;
                    }
                    // TODO: Better error handling
                    _ => unreachable!(),
                }

                // SAFETY: The multi-version data structure should not leak an index over block size.
                let tx_result = unsafe { fully_evaluated_results.get_unchecked_mut(tx_idx) };
                let account = tx_result.state.entry(address).or_default();
                // TODO: Deduplicate this logic with [PevmTxExecutionResult::from_revm]
                if spec_id.is_enabled_in(SPURIOUS_DRAGON)
                    && code_hash.is_none()
                    && current_account.nonce == 0
                    && current_account.balance == U256::ZERO
                {
                    *account = None;
                } else if let Some(account) = account {
                    // Explicit write: only overwrite the account info in case there are storage changes
                    // TODO: Can code be changed mid-block?
                    account.basic.balance = current_account.balance;
                    account.basic.nonce = current_account.nonce;
                } else {
                    // Implicit write: e.g. gas payments to the beneficiary account,
                    // which doesn't have explicit writes in [tx_result.state]
                    *account = Some(EvmAccount {
                        basic: current_account.clone(),
                        code_hash,
                        code: code.clone(),
                        storage: AHashMap::default(),
                    });
                }
            }
        }
    }

    Ok(fully_evaluated_results)
}

fn try_execute<S: Storage, C: PevmChain>(
    mv_memory: &MvMemory,
    vm: &Vm<S, C>,
    scheduler: &Scheduler,
    abort_reason: &OnceLock<AbortReason>,
    execution_results: &[Mutex<Option<PevmTxExecutionResult>>],
    tx_version: TxVersion,
) -> Option<Task> {
    loop {
        return match vm.execute(tx_version.tx_idx) {
            VmExecutionResult::Retry => {
                if abort_reason.get().is_none() {
                    continue;
                }
                None
            }
            VmExecutionResult::FallbackToSequential => {
                scheduler.abort();
                abort_reason.get_or_init(|| AbortReason::FallbackToSequential);
                None
            }
            VmExecutionResult::ReadError { blocking_tx_idx } => {
                if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx)
                    && abort_reason.get().is_none()
                {
                    // Retry the execution immediately if the blocking transaction was
                    // re-executed by the time we can add it as a dependency.
                    continue;
                }
                None
            }
            VmExecutionResult::ExecutionError(err) => {
                scheduler.abort();
                abort_reason.get_or_init(|| AbortReason::ExecutionError(err));
                None
            }
            VmExecutionResult::Ok {
                execution_result,
                read_set,
                write_set,
                lazy_addresses,
                next_validation_idx,
            } => {
                *index_mutex!(execution_results, tx_version.tx_idx) = Some(execution_result);
                let wrote_new_location =
                    mv_memory.record(&tx_version, read_set, write_set, lazy_addresses);
                scheduler.finish_execution(tx_version, wrote_new_location, next_validation_idx)
            }
        };
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
