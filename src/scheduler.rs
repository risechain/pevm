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

use crate::{ExecutionTask, Task, TxIdx, TxIncarnation, TxVersion, ValidationTask};

// - ReadyToExecute(i) --try_incarnate--> Executing(i)
// Non-blocked execution:
//   - Executing(i) --finish_execution--> Executed(i)
//   - Executed(i) --try_validation_abort--> Aborting(i)
//   - Aborted(i) --finish_validation(w.aborted=true)--> ReadyToExecute(i+1)
// Blocked execution:
//   - Executing(i) --add_dependency--> Aborting(i)
//   - Aborting(i) --resume--> ReadyToExecute(i+1)
#[derive(PartialEq, Debug)]
pub(crate) enum TxIncarnationStatus {
    ReadyToExecute(TxIncarnation),
    Executing(TxIncarnation),
    Executed(TxIncarnation),
    Aborting(TxIncarnation),
}

// The BlockSTM collaborative scheduler coordinates execution & validation
// tasks among threads.
//
// The task sets are implemented via an atomic counter coupled with a
// mechanism to track the status of transactions, i.e., whether a given
// transaction is ready for validation or execution.
// To pick a task, threads increment the smaller of these counters until
// they find a task that is ready to be performed. To add a task for
// a transaction, the thread updates the status and reduces the
// corresponding counter to the transaction index if it had a larger value.
//
// An incarnation might write to a memory location that was previously
// read by an incarnation of a higher transaction. This is why when an
// incarnation finishes, new validation tasks are created for higher
// transactions. Importantly, validation tasks are scheduled optimistically.
// When threads are available, BlockSTM capitalizes by performing these
// validation in parallel. Identifying validation failures and aborting
// incarnations as soon as possible is cruicial for the system performance,
// as any incarnation that reads values written by an incarnation that aborts
// also must abort, forming a cascade of aborts.
//
// When an incarnation writes only to a subset of memory locations written
// by the previously completed incarnation of the same transaction, BlockSTM
// schedules validation just for the incarnation. This is sufficient as the
// whole write set of the previous incarnation is marked as ESTIMATE during
// the abort. The abort leas to optimistically creating validation tasks
// for higher transactions. Threads that perform these tasks can already
// detect validation failure due to the ESTIMATE markers on memory locations,
// instead of waiting for a subsequent incarnation to finish.
//
// The Scheduler contains the shared variables and logic used to dispatch
// execution & validation tasks.
pub(crate) struct Scheduler {
    /// The number of transactions in this block.
    block_size: usize,
    /// The next transaction to try and execute.
    execution_idx: AtomicUsize,
    /// The next tranasction to try and validate.
    validation_idx: AtomicUsize,
    /// Number of times execution or validation indices were decreased.
    decrease_cnt: AtomicUsize,
    /// The most up-to-date incarnation number (initially 0) amd
    /// the status of this incarnation.
    transactions_status: Vec<Mutex<TxIncarnationStatus>>,
    transactions_dependencies: Vec<Mutex<Vec<TxIdx>>>,
    /// Number of ongoing execution and validation tasks.
    num_active_tasks: AtomicUsize,
    /// Marker for completion
    done_marker: AtomicBool,
}

impl Scheduler {
    pub(crate) fn new(block_size: usize) -> Self {
        Self {
            block_size,
            execution_idx: AtomicUsize::new(0),
            validation_idx: AtomicUsize::new(0),
            decrease_cnt: AtomicUsize::new(0),
            transactions_status: (0..block_size)
                .map(|_| Mutex::new(TxIncarnationStatus::ReadyToExecute(0)))
                .collect(),
            transactions_dependencies: (0..block_size).map(|_| Mutex::new(Vec::new())).collect(),
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

    fn try_incarnate(&self, tx_idx: TxIdx) -> Option<TxVersion> {
        // TODO: Better error handling
        if tx_idx < self.block_size {
            let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
            if let TxIncarnationStatus::ReadyToExecute(i) = *transaction_status {
                let tx_incarnation = i.clone();
                *transaction_status = TxIncarnationStatus::Executing(tx_incarnation);
                return Some(TxVersion {
                    tx_idx,
                    tx_incarnation,
                });
            }
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
        let validation_idx = self.validation_idx.fetch_add(1, Relaxed);
        if validation_idx < self.block_size {
            // TODO: Better error handling
            if let TxIncarnationStatus::Executed(i) =
                *self.transactions_status[validation_idx].lock().unwrap()
            {
                return Some(TxVersion {
                    tx_idx: validation_idx,
                    tx_incarnation: i.clone(),
                });
            }
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }

    pub(crate) fn next_task(&self) -> Option<Task> {
        if self.validation_idx.load(Relaxed) < self.execution_idx.load(Relaxed) {
            match self.next_version_to_validate() {
                Some(tx_version) => Some(Task::Validation(tx_version)),
                _ => None,
            }
        } else {
            match self.next_version_to_execute() {
                Some(tx_version) => Some(Task::Execution(tx_version)),
                _ => None,
            }
        }
    }

    // Add `tx_idx` as the dependency of `blocking_tx_idx` so `tx_idx` is
    // re-executed when the next `blocking_tx_idx` is executed.
    // Return `false` if we encouter a race condition when `blocking_tx_idx`
    // gets re-executed before the dependency can be added.
    // TODO: Better error handling, including asserting that both indices are in range
    pub(crate) fn add_dependency(&self, tx_idx: TxIdx, blocking_tx_idx: TxIdx) -> bool {
        // NOTE: This is an important lock to prevent a race condition where the blocking
        // transaction completes execution before this dependecy can be added.
        let blocking_transaction_status = self.transactions_status[blocking_tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executed(_) = *blocking_transaction_status {
            return false;
        }

        let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executing(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::Aborting(i);

            // TODO: Better error handling here
            let mut blocking_dependencies = self.transactions_dependencies[blocking_tx_idx]
                .lock()
                .unwrap();
            blocking_dependencies.push(tx_idx);

            self.num_active_tasks.fetch_sub(1, Relaxed);
            return true;
        }

        unreachable!("Trying to abort & add dependency in non-executing state!")
    }

    // TODO: Better error handling
    // Be careful as this one is usually called as a sub-routine that is very
    // dead-lock prone.
    fn set_ready_status(&self, tx_idx: TxIdx) {
        let mut transaction_status = self.transactions_status[tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Aborting(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::ReadyToExecute(i + 1)
        } else {
            unreachable!("Trying to resume in non-aborting state!")
        }
    }

    // When a new location was written, schedule the re-execution of all
    // higher transactions. If not, return the validation task to validate
    // only this transaction.
    pub(crate) fn finish_execution(
        &self,
        tx_version: &TxVersion,
        wrote_new_location: bool,
    ) -> Option<ValidationTask> {
        // TODO: Better error handling
        let mut transaction_status = self.transactions_status[tx_version.tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executing(i) = *transaction_status {
            // TODO: Assert that `i` equals `tx_version.tx_incarnation`?
            *transaction_status = TxIncarnationStatus::Executed(i);

            // TODO: Better error handling
            let mut dependencies = self.transactions_dependencies[tx_version.tx_idx]
                .lock()
                .unwrap();

            // Resume dependent transactions
            let mut min_dependency_idx = None;
            for tx_idx in dependencies.clone() {
                self.set_ready_status(tx_idx);
                min_dependency_idx = match min_dependency_idx {
                    None => Some(tx_idx),
                    Some(min_index) => Some(min(tx_idx, min_index)),
                }
            }
            dependencies.clear();

            if let Some(min_idx) = min_dependency_idx {
                self.decrease_execution_idx(min_idx);
            }

            if self.validation_idx.load(Relaxed) > tx_version.tx_idx {
                if wrote_new_location {
                    self.decrease_validation_idx(tx_version.tx_idx);
                } else {
                    return Some(tx_version.clone());
                }
            }
        } else {
            // TODO: Better error handling
            unreachable!("Trying to finish execution in non-executing states")
        }
        self.num_active_tasks.fetch_sub(1, Relaxed);
        None
    }

    // Return whether the abort was successful. The scheduler ensures that only
    // one failing validation per version may lead to a successful abort.
    // Return `false` if the incarnation was already aborted. A successful abort
    // leads to scheduling the transaction for re-execution and the higher
    // transactions for validation during `finish_validation`.
    pub(crate) fn try_validation_abort(&self, tx_version: &TxVersion) -> bool {
        // TODO: Better error handling
        let mut transaction_status = self.transactions_status[tx_version.tx_idx].lock().unwrap();
        if let TxIncarnationStatus::Executed(i) = *transaction_status {
            *transaction_status = TxIncarnationStatus::Aborting(i.clone());
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
