use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
};

use revm::primitives::{ResultAndState, TxEnv};

use crate::{
    mv_memory::MvMemory,
    scheduler::Scheduler,
    storage::Storage,
    vm::{Vm, VmExecutionResult},
    ExecutionTask, Task, TxVersion, ValidationTask,
};

// TODO: Better design & API
pub struct BlockSTM;

impl BlockSTM {
    // TODO: Better concurrency control
    pub fn run(
        storage: Storage,
        txs: Arc<Vec<TxEnv>>,
        concurrency_level: NonZeroUsize,
    ) -> Vec<ResultAndState> {
        let block_size = txs.len();
        let scheduler = Scheduler::new(block_size);
        let mv_memory = Arc::new(MvMemory::new(block_size));
        let vm = Vm::new(Arc::new(storage), txs.clone(), mv_memory.clone());
        let execution_results = Mutex::new(vec![None; txs.len()]);
        // TODO: Better thread handling
        thread::scope(|scope| {
            for _ in 0..concurrency_level.into() {
                scope.spawn(|| {
                    let mut task = None;
                    while !scheduler.done() {
                        // Perform the task with the smallest transaction index:
                        // 1. Execution: Execute the next incarnation. If a value marked
                        //   as ESTIMATE is read, abort execution and add the transaction
                        //   back to the execution tasks. Otherwise:
                        //   (a) If there is a write to a memory location to which the
                        //     previous finished incarnation has not written, create validation
                        //     tasks for all higher transactions not currently in execution
                        //     tasks or being executed and add them to the validation tasks.
                        //   (b) Otherwise, create a validation task only for the transaction.
                        // 2. Validation: Validate the last incarnation of the transaction.
                        //   If validation succeeds, continue. Otherwise, abort:
                        //   (a) Mark every value (in the multi-version data-structure) written
                        //     by the incarnation (that failed validation) as an ESTIMATE.
                        //   (b) Create validation tasks for all higher transactions that are
                        //     not currently in execution tasks or being executed and add them
                        //     to validation tasks.
                        //   (c) Create an execution task for the transaction with an incremented
                        //     incarnation number, and add it to execution tasks.
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
                                needs_reexecution(&mv_memory, &scheduler, &tx_version)
                                    .map(Task::Execution)
                            }
                            None => scheduler.next_task(),
                        };
                    }
                });
            }
        });

        println!(
            "MV Memory snapshot length: {}",
            // TODO: Better error handling
            mv_memory.snapshot().len()
        );

        // TODO: Better error handling
        let execution_results = execution_results.lock().unwrap();
        execution_results
            .iter()
            .cloned()
            // TODO: Better error handling
            // Scheduler shouldn't claim `done` when
            // there is still a `None`` result.
            .map(|r| r.unwrap())
            .collect()
    }
}

// Process an execution task
fn try_execute(
    mv_memory: &Arc<MvMemory>,
    vm: &Vm,
    scheduler: &Scheduler,
    execution_results: &Mutex<Vec<Option<ResultAndState>>>,
    tx_version: &TxVersion,
) -> Option<ValidationTask> {
    match vm.execute(tx_version.tx_idx) {
        VmExecutionResult::ReadError { blocking_tx_idx } => {
            // Retry the execution immediately if the blocking transaction was
            // re-executed by the time we can add it as a dependency.
            if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx) {
                return try_execute(mv_memory, vm, scheduler, execution_results, tx_version);
            }
            None
        }
        VmExecutionResult::Ok {
            result_and_state,
            read_set,
            write_set,
        } => {
            execution_results.lock().unwrap()[tx_version.tx_idx] = Some(result_and_state);
            let wrote_new_location = mv_memory.record(tx_version, read_set, write_set);
            scheduler.finish_execution(tx_version, wrote_new_location)
        }
    }
}

// May return a re-execution task
fn needs_reexecution(
    mv_memory: &Arc<MvMemory>,
    scheduler: &Scheduler,
    tx_version: &TxVersion,
) -> Option<ExecutionTask> {
    let read_set_valid = mv_memory.validate_read_set(tx_version.tx_idx);
    let aborted = !read_set_valid && scheduler.try_validation_abort(&tx_version);
    if aborted {
        mv_memory.convert_writes_to_estimates(tx_version.tx_idx);
    }
    scheduler.finish_validation(tx_version, aborted)
}
