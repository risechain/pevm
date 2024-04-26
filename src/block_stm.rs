use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
};

use revm::primitives::{BlockEnv, ResultAndState, TxEnv};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    scheduler::Scheduler,
    vm::{Vm, VmExecutionResult},
    ExecutionTask, MemoryLocation, MemoryValue, Storage, Task, TxVersion, ValidationTask,
};

/// An interface to execute Block-STM.
/// TODO: Better design & API.
#[derive(Debug)]
pub struct BlockSTM;

impl BlockSTM {
    /// Run a list of REVM transactions through Block-STM.
    /// TODO: Better concurrency control
    pub fn run(
        storage: Storage,
        block_env: BlockEnv,
        txs: Vec<TxEnv>,
        concurrency_level: NonZeroUsize,
    ) -> Vec<ResultAndState> {
        let block_size = txs.len();
        let scheduler = Scheduler::new(block_size);
        let mv_memory = Arc::new(MvMemory::new(block_size));
        // TODO: Better error handling
        let mut beneficiary_account_info = storage.basic(block_env.coinbase).unwrap();
        let vm = Vm::new(storage, block_env.clone(), txs, mv_memory.clone());
        // TODO: Should we move this to `Vm`?
        let execution_results = (0..block_size).map(|_| Mutex::new(None)).collect();
        // TODO: Better thread handling
        thread::scope(|scope| {
            for _ in 0..concurrency_level.into() {
                scope.spawn(|| {
                    let mut task = None;
                    while !scheduler.done() {
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
                                &execution_results,
                                &tx_version,
                            )
                            .map(Task::Validation),
                            Some(Task::Validation(tx_version)) => {
                                try_validate(&mv_memory, &scheduler, &tx_version)
                                    .map(Task::Execution)
                            }
                            None => scheduler.next_task(),
                        };
                    }
                });
            }
        });

        // We lazily evaluate the final beneficiary account's balance at the end of each transaction
        // to avoid "implicit" dependency among consecutive transactions that read & write there.
        // TODO: Refactor, improve speed & error handling.
        execution_results
            .iter()
            .map(|m| m.lock().unwrap().clone().unwrap())
            .enumerate()
            .map(|(tx_idx, mut result_and_state)| {
                // TODO: Support pre-EIP-3651 when the beneficiary account may not be loaded and be
                // present in the state changes map.
                let beneficiary_account =
                    result_and_state.state.get_mut(&block_env.coinbase).unwrap();
                beneficiary_account.mark_touch();
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
                beneficiary_account.info = beneficiary_account_info.clone();
                result_and_state
            })
            .collect()
    }
}

// Execute the next incarnation of a transaction.
// If an ESTIMATE is read, abort and add dependency for re-execution.
// Otherwise:
// - If there is a write to a memory location to which the
//   previous finished incarnation has not written, create validation
//   tasks for all higher transactions.
// - Otherwise, return a validation task for the transaction.
fn try_execute(
    mv_memory: &Arc<MvMemory>,
    vm: &Vm,
    scheduler: &Scheduler,
    execution_results: &Vec<Mutex<Option<ResultAndState>>>,
    tx_version: &TxVersion,
) -> Option<ValidationTask> {
    match vm.execute(tx_version.tx_idx) {
        VmExecutionResult::ReadError { blocking_tx_idx } => {
            if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx) {
                // Retry the execution immediately if the blocking transaction was
                // re-executed by the time we can add it as a dependency.
                return try_execute(mv_memory, vm, scheduler, execution_results, tx_version);
            }
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
