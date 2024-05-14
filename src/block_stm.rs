use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, RwLock},
    thread,
};

use alloy_rpc_types::{Block, Header};
use revm::primitives::{Account, AccountInfo, BlockEnv, ResultAndState, SpecId, TxEnv};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    primitives::{get_block_env, get_block_spec, get_tx_envs},
    scheduler::Scheduler,
    vm::{ExecutionError, Vm, VmExecutionResult},
    ExecutionTask, MemoryLocation, MemoryValue, Storage, Task, TxVersion, ValidationTask,
};

/// Errors when executing a block with BlockSTM.
#[derive(Debug)]
pub enum BlockStmError {
    /// Cannot derive the chain spec from the block header.
    UnknownBlockSpec,
    /// Block header lacks information for execution.
    MissingHeaderData,
    /// Transactions lack information for execution.
    MissingTransactionData,
    /// EVM execution error.
    ExecutionError(ExecutionError),
}

/// Execution result of BlockSTM.
pub type BlockStmResult = Result<Vec<ResultAndState>, BlockStmError>;

/// Execute an Alloy block, which is becoming the "standard" format in Rust.
pub fn execute<S: Storage + Send + Sync>(
    storage: S,
    block: Block,
    parent_header: Option<Header>,
    concurrency_level: NonZeroUsize,
) -> BlockStmResult {
    let Some(spec_id) = get_block_spec(&block.header) else {
        return Err(BlockStmError::UnknownBlockSpec);
    };
    let Some(block_env) = get_block_env(&block.header, parent_header.as_ref()) else {
        return Err(BlockStmError::MissingHeaderData);
    };
    let Some(tx_envs) = get_tx_envs(&block.transactions) else {
        return Err(BlockStmError::MissingTransactionData);
    };
    execute_revm(storage, spec_id, block_env, tx_envs, concurrency_level)
}

/// Execute an REVM block.
pub fn execute_revm<S: Storage + Send + Sync>(
    storage: S,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
    concurrency_level: NonZeroUsize,
) -> BlockStmResult {
    let block_size = txs.len();
    let scheduler = Scheduler::new(&txs);
    let mv_memory = Arc::new(MvMemory::new(block_size));
    let mut beneficiary_account_info = match storage.basic(block_env.coinbase) {
        Ok(Some(account)) => account.into(),
        _ => AccountInfo::default(),
    };
    let vm = Vm::new(storage, spec_id, block_env.clone(), txs, mv_memory.clone());

    // TODO: Should we move this to `Vm`?
    let execution_error = RwLock::new(None);
    let execution_results = (0..block_size).map(|_| Mutex::new(None)).collect();

    // TODO: Better thread handling
    thread::scope(|scope| {
        for _ in 0..concurrency_level.into() {
            scope.spawn(|| {
                let mut task = None;
                while !scheduler.done() {
                    // TODO: Have different functions or an enum for the caller to choose
                    // the handling behaviour when a transaction's EVM execution fails.
                    // Parallel block builders would like to exclude such transaction,
                    // verifiers may want to exit early to save CPU cycles, while testers
                    // may want to collect all execution results. We are exiting early as
                    // the default behaviour for now.
                    if execution_error.read().unwrap().is_some() {
                        break;
                    }

                    // Find and perform the next execution or validation task.
                    //
                    // After an incarnation executes it needs to pass validation. The
                    // validation re-reads the read-set and compares the observed versions.
                    // A successful validation implies that the applied writes are still
                    // up-to-date. A failed validation means the incarnation must be
                    // aborted and the transaction is re-executed in a next one.
                    //
                    // A successful validation does not guarantee that an incarnation can be
                    // committed. Since an abortion and re-execution of an earlier transaction
                    // in the block might invalidate the incarnation read set and necessitate
                    // re-execution. Thus, when a transaction aborts, all higher transactions
                    // are scheduled for re-validation. The same incarnation may be validated
                    // multiple times, by different threads, and potentially in parallel, but
                    // BlockSTM ensures that only the first abort per version succeeds.
                    //
                    // Since transactions must be committed in order, BlockSTM prioritizes
                    // tasks associated with lower-indexed transactions.
                    task = match task {
                        Some(Task::Execution(tx_version)) => try_execute(
                            &mv_memory,
                            &vm,
                            &scheduler,
                            &execution_error,
                            &execution_results,
                            &tx_version,
                        )
                        .map(Task::Validation),
                        Some(Task::Validation(tx_version)) => {
                            try_validate(&mv_memory, &scheduler, &tx_version).map(Task::Execution)
                        }
                        None => scheduler.next_task(),
                    };
                }
            });
        }
    });

    if let Some(err) = execution_error.read().unwrap().as_ref() {
        return Err(BlockStmError::ExecutionError(err.clone()));
    }

    // We lazily evaluate the final beneficiary account's balance at the end of each transaction
    // to avoid "implicit" dependency among consecutive transactions that read & write there.
    // TODO: Refactor, improve speed & error handling.
    Ok(execution_results
        .iter()
        .map(|m| m.lock().unwrap().clone().unwrap())
        .enumerate()
        .map(|(tx_idx, mut result_and_state)| {
            match mv_memory.read_absolute(&MemoryLocation::Basic(block_env.coinbase), tx_idx) {
                ReadMemoryResult::Ok {
                    value: MemoryValue::Basic(account),
                    ..
                } => {
                    beneficiary_account_info = account;
                }
                ReadMemoryResult::Ok {
                    value: MemoryValue::LazyBeneficiaryBalance(addition),
                    ..
                } => {
                    beneficiary_account_info.balance += addition;
                }
                _ => unreachable!(),
            }
            let mut beneficiary_account = Account::from(beneficiary_account_info.clone());
            beneficiary_account.mark_touch();
            result_and_state
                .state
                .insert(block_env.coinbase, beneficiary_account);
            result_and_state
        })
        .collect())
}

// Execute the next incarnation of a transaction.
// If an ESTIMATE is read, abort and add dependency for re-execution.
// Otherwise:
// - If there is a write to a memory location to which the
//   previous finished incarnation has not written, create validation
//   tasks for all higher transactions.
// - Otherwise, return a validation task for the transaction.
fn try_execute<S: Storage>(
    mv_memory: &Arc<MvMemory>,
    vm: &Vm<S>,
    scheduler: &Scheduler,
    execution_error: &RwLock<Option<ExecutionError>>,
    execution_results: &Vec<Mutex<Option<ResultAndState>>>,
    tx_version: &TxVersion,
) -> Option<ValidationTask> {
    match vm.execute(tx_version.tx_idx) {
        VmExecutionResult::ReadError { blocking_tx_idx } => {
            if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx) {
                // Retry the execution immediately if the blocking transaction was
                // re-executed by the time we can add it as a dependency.
                return try_execute(
                    mv_memory,
                    vm,
                    scheduler,
                    execution_error,
                    execution_results,
                    tx_version,
                );
            }
            None
        }
        VmExecutionResult::ExecutionError(err) => {
            *execution_error.write().unwrap() = Some(err);
            None
        }
        VmExecutionResult::Ok {
            result_and_state,
            read_set,
            write_set,
        } => {
            *execution_results[tx_version.tx_idx].lock().unwrap() = Some(result_and_state);
            let wrote_new_location = mv_memory.record(tx_version, read_set, write_set);
            scheduler.finish_execution(tx_version, wrote_new_location)
        }
    }
}

// Validate the last incarnation of the transaction.
// If validation fails:
// - Mark every memory value written by the incarnation as ESTIMATE.
// - Create validation tasks for all higher transactions that have
//   not been executed.
// - Return a re-execution task for this transaction with an incremented
//   incarnation.
fn try_validate(
    mv_memory: &Arc<MvMemory>,
    scheduler: &Scheduler,
    tx_version: &TxVersion,
) -> Option<ExecutionTask> {
    let read_set_valid = mv_memory.validate_read_set(tx_version.tx_idx);
    let aborted = !read_set_valid && scheduler.try_validation_abort(tx_version);
    if aborted {
        mv_memory.convert_writes_to_estimates(tx_version.tx_idx);
    }
    scheduler.finish_validation(tx_version, aborted)
}
