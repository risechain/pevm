use std::{
    cmp::min,
    sync::{
        // TODO: Fine-tune all atomic `Ordering`s.
        // We're starting with `Relaxed` for maximum performance
        // without any issue so far. When in trouble, we can
        // retry `SeqCst` for robustness.
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
        Mutex,
    },
};

use ahash::{AHashMap, AHashSet};

use crate::{
    ExecutionTask, Task, TransactionsDependencies, TransactionsDependents, TransactionsStatus,
    TxIdx, TxIncarnationStatus, TxVersion, ValidationTask,
};

// The BlockSTM collaborative scheduler coordinates execution & validation
// tasks among threads.
//
// To pick a task, threads increment the smaller of the (execution and
// validation) task counters until they find a task that is ready to be
// performed. To redo a task for a transaction, the thread updates the status
// and reduces the corresponding counter to the transaction index if it had a
// larger value.
//
// An incarnation may write to a memory location that was previously
// read by a higher transaction. Thus, when an incarnation finishes, new
// validation tasks are created for higher transactions.
//
// Validation tasks are scheduled optimistically and in parallel. Identifying
// validation failures and aborting incarnations as soon as possible is critical
// for performance, as any incarnation that reads values written by an
// incarnation that aborts also must abort.
// When an incarnation writes only to a subset of memory locations written
// by the previously completed incarnation of the same transaction, we schedule
// validation just for the incarnation. This is sufficient as the whole write
// set of the previous incarnation is marked as ESTIMATE during the abort.
// The abort leads to optimistically creating validation tasks for higher
// transactions. Threads that perform these tasks can already detect validation
// failure due to the ESTIMATE markers on memory locations, instead of waiting
// for a subsequent incarnation to finish.
pub(crate) struct Scheduler {
    /// The number of transactions in this block.
    block_size: usize,
    /// The next transaction to try and execute.
    execution_idx: AtomicUsize,
    /// The next tranasction to try and validate.
    validation_idx: AtomicUsize,
    /// Number of times a task index was decreased.
    decrease_cnt: AtomicUsize,
    /// The most up-to-date incarnation number (initially 0) and
    /// the status of this incarnation.
    transactions_status: Vec<Mutex<TxIncarnationStatus>>,
    /// The list of dependent transactions to resumne when the
    /// key transaction is re-executed.
    transactions_dependents: Vec<Mutex<AHashSet<TxIdx>>>,
    /// A list of optional dependencies flagged during preprocessing.
    /// For instance, for a transaction to depend on two lower others,
    /// one send to the same recipient address, and one is from
    /// the same sender. We cannot casually check if all dependencies
    /// are clear with the dependents map as it can only lock the
    /// dependency. Two dependencies may check at the same time
    /// before they clear and think that the dependent is not yet
    /// ready, making it forever unexecuted.
    // TODO: Build a fuller dependency graph.
    transactions_dependencies: AHashMap<TxIdx, Mutex<AHashSet<TxIdx>>>,
    /// Number of ongoing execution and validation tasks.
    num_active_tasks: AtomicUsize,
    /// Marker for completion
    done_marker: AtomicBool,
}

impl Scheduler {
    pub(crate) fn new(
        block_size: usize,
        transactions_status: TransactionsStatus,
        transactions_dependents: TransactionsDependents,
        transactions_dependencies: TransactionsDependencies,
    ) -> Self {
        Self {
            block_size,
            execution_idx: AtomicUsize::new(0),
            validation_idx: AtomicUsize::new(0),
            decrease_cnt: AtomicUsize::new(0),
            transactions_status: transactions_status.into_iter().map(Mutex::new).collect(),
            transactions_dependents: transactions_dependents
                .into_iter()
                .map(Mutex::new)
                .collect(),
            transactions_dependencies: transactions_dependencies
                .into_iter()
                .map(|(tx_idx, deps)| (tx_idx, Mutex::new(deps)))
                .collect(),
            num_active_tasks: AtomicUsize::new(0),
            done_marker: AtomicBool::new(false),
        }
    }

    pub(crate) fn done(&self) -> bool {
        self.done_marker.load(Relaxed)
    }

    fn decrease_execution_idx(&self, target_idx: TxIdx) {
        if self.execution_idx.fetch_min(target_idx, Relaxed) > target_idx {
            self.decrease_cnt.fetch_add(1, Relaxed);
        }
    }

    fn decrease_validation_idx(&self, target_idx: TxIdx) {
        if self.validation_idx.fetch_min(target_idx, Relaxed) > target_idx {
            self.decrease_cnt.fetch_add(1, Relaxed);
        }
    }

    fn check_done(&self) {
        let observed_cnt = self.decrease_cnt.load(Relaxed);
        let execution_idx = self.execution_idx.load(Relaxed);
        let validation_idx = self.validation_idx.load(Relaxed);
        let num_active_tasks = self.num_active_tasks.load(Relaxed);
        if min(execution_idx, validation_idx) >= self.block_size
            && num_active_tasks == 0
            && observed_cnt == self.decrease_cnt.load(Relaxed)
        {
            self.done_marker.store(true, Relaxed);
        }
    }

    fn try_incarnate(&self, mut tx_idx: TxIdx) -> Option<TxVersion> {
        while tx_idx < self.block_size {
            let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
            if let TxIncarnationStatus::ReadyToExecute(i) = *transaction_status {
                *transaction_status = TxIncarnationStatus::Executing(i);
                return Some(TxVersion {
                    tx_idx,
                    tx_incarnation: i,
                });
            }
            drop(transaction_status);
            tx_idx = self.execution_idx.fetch_add(1, Relaxed);
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }

    fn next_version_to_execute(&self) -> Option<TxVersion> {
        if self.execution_idx.load(Relaxed) >= self.block_size {
            self.check_done();
            None
        } else {
            self.num_active_tasks.fetch_add(1, Relaxed);
            self.try_incarnate(self.execution_idx.fetch_add(1, Relaxed))
        }
    }

    fn next_version_to_validate(&self) -> Option<TxVersion> {
        if self.validation_idx.load(Relaxed) >= self.block_size {
            self.check_done();
            return None;
        }
        self.num_active_tasks.fetch_add(1, Relaxed);
        let mut validation_idx = self.validation_idx.fetch_add(1, Relaxed);
        while validation_idx < self.block_size {
            let transaction_status = self.transactions_status[validation_idx].lock().unwrap();
            if let TxIncarnationStatus::Executed(i) = *transaction_status {
                return Some(TxVersion {
                    tx_idx: validation_idx,
                    tx_incarnation: i,
                });
            }
            drop(transaction_status);
            validation_idx = self.validation_idx.fetch_add(1, Relaxed);
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }

    pub(crate) fn next_task(&self) -> Option<Task> {
        if self.validation_idx.load(Relaxed) < self.execution_idx.load(Relaxed) {
            self.next_version_to_validate().map(Task::Validation)
        } else {
            self.next_version_to_execute().map(Task::Execution)
        }
    }

    // Add `tx_idx` as a dependent of `blocking_tx_idx` so `tx_idx` is
    // re-executed when the next `blocking_tx_idx` incarnation is executed.
    // Return `false` if we encouter a race condition when `blocking_tx_idx`
    // gets re-executed before the dependency can be added.
    // TODO: Better error handling, including asserting that both indices are in range.
    pub(crate) fn add_dependency(&self, tx_idx: TxIdx, blocking_tx_idx: TxIdx) -> bool {
        // This is an important lock to prevent a race condition where the blocking
        // transaction completes re-execution before this dependecy can be added.
        let blocking_transaction_status = self.transactions_status[blocking_tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executed(_) = *blocking_transaction_status {
            return false;
        }

        let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executing(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::Aborting(i);
            drop(transaction_status);

            // TODO: Better error handling here
            let mut blocking_dependents = self.transactions_dependents[blocking_tx_idx]
                .lock()
                .unwrap();
            blocking_dependents.insert(tx_idx);
            drop(blocking_dependents);

            self.num_active_tasks.fetch_sub(1, Relaxed);
            return true;
        }

        unreachable!("Trying to abort & add dependency in non-executing state!")
    }

    // Be careful as this one is usually called as a sub-routine that is very
    // easy to dead-lock.
    fn set_ready_status(&self, tx_idx: TxIdx) {
        // TODO: Better error handling
        let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Aborting(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::ReadyToExecute(i + 1)
        } else {
            unreachable!("Trying to resume in non-aborting state!")
        }
    }

    // Finish execution and resume dependents of a transaction incarnation.
    // When a new location was written, schedule the re-execution of all
    // higher transactions. If not, return the validation task to validate
    // only this incarnation. Return no task if we've already rolled back to
    // re-validating smaller transactions.
    pub(crate) fn finish_execution(
        &self,
        tx_version: TxVersion,
        wrote_new_location: bool,
    ) -> Option<ValidationTask> {
        // TODO: Better error handling
        let mut transaction_status = self.transactions_status[tx_version.tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executing(i) = *transaction_status {
            // TODO: Assert that `i` equals `tx_version.tx_incarnation`?
            *transaction_status = TxIncarnationStatus::Executed(i);
            drop(transaction_status);

            // TODO: Better error handling
            let mut dependents = self.transactions_dependents[tx_version.tx_idx]
                .lock()
                .unwrap();

            // Resume dependent transactions
            let mut min_dependent_idx = None;
            for tx_idx in dependents.iter() {
                if let Some(deps) = self.transactions_dependencies.get(tx_idx) {
                    // TODO: Better error handling
                    let mut deps = deps.lock().unwrap();
                    deps.retain(|dep_idx| dep_idx != &tx_version.tx_idx);
                    // Skip this dependent as it has other pending dependencies.
                    // Let the last one evoke it.
                    if !deps.is_empty() {
                        continue;
                    }
                }
                self.set_ready_status(*tx_idx);
                min_dependent_idx = match min_dependent_idx {
                    None => Some(*tx_idx),
                    Some(min_index) => Some(min(*tx_idx, min_index)),
                }
            }
            dependents.clear();
            drop(dependents);

            if let Some(min_idx) = min_dependent_idx {
                self.decrease_execution_idx(min_idx);
            }

            if self.validation_idx.load(Relaxed) > tx_version.tx_idx {
                // This incarnation wrote to a new location, so we must
                // re-evaluate it (via the immediately returned task returned
                // immediately) and all higher transactions in case they read
                // the new location.
                if wrote_new_location {
                    self.decrease_validation_idx(tx_version.tx_idx + 1);
                }
                return Some(tx_version);
            }
        } else {
            // TODO: Better error handling
            unreachable!("Trying to finish execution in a non-executing state")
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }

    // Return whether the abort was successful. A successful abort leads to
    // scheduling the transaction for re-execution and the higher transactions
    // for validation during `finish_validation`. The scheduler ensures that only
    // one failing validation per version can lead to a successful abort.
    pub(crate) fn try_validation_abort(&self, tx_version: &TxVersion) -> bool {
        // TODO: Better error handling
        let mut transaction_status = self.transactions_status[tx_version.tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executed(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::Aborting(i);
            true
        } else {
            false
        }
    }

    // When there is a successful abort, schedule the transaction for re-execution
    // and the higher transactions for validation. The re-execution task is returned
    // for the aborted transaction.
    pub(crate) fn finish_validation(
        &self,
        tx_version: &TxVersion,
        aborted: bool,
    ) -> Option<ExecutionTask> {
        if aborted {
            self.set_ready_status(tx_version.tx_idx);
            self.decrease_validation_idx(tx_version.tx_idx + 1);
            if self.execution_idx.load(Relaxed) > tx_version.tx_idx {
                return self.try_incarnate(tx_version.tx_idx);
            }
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }
}
