use std::{
    cmp::min,
    sync::{
        // TODO: Fine-tune all atomic `Ordering`s.
        // We're starting with `Relaxed` for maximum performance
        // without any issue so far. When in trouble, we can
        // retry `SeqCst` for robustness.
        atomic::{AtomicUsize, Ordering::Relaxed},
        Mutex,
    },
};

use ahash::AHashMap;
use crossbeam::utils::CachePadded;

use crate::{
    ExecutionTask, IncarnationStatus, Task, TransactionsDependencies, TransactionsDependents,
    TransactionsStatus, TxIdx, TxStatus, TxVersion, ValidationTask,
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
//
// To fight false-sharing, instead of blindly padding everything, we run
// $ CARGO_PROFILE_BENCH_DEBUG=true cargo bench --bench mainnet
// $ perf record -e cache-misses target/release/deps/mainnet-??? --bench
// $ perf report
// To identify then pad atomics with the highest overheads.
// Current tops:
// - `execution_idx`
// - `validation_idx`
// - The ones inside `transactions_status` `Mutex`es
// We also align the struct and each field up to `transactions_status`
// to start at a new 64-or-128-bytes cache line.
#[repr(align(128))]
pub(crate) struct Scheduler {
    /// The next transaction to try and execute.
    execution_idx: CachePadded<AtomicUsize>,
    /// The next transaction to try and validate.
    validation_idx: CachePadded<AtomicUsize>,
    /// The most up-to-date incarnation number (initially 0) and
    /// the status of this incarnation.
    transactions_status: Vec<CachePadded<Mutex<TxStatus>>>,
    /// The number of transactions in this block.
    block_size: usize,
    // We won't validate until we find the first transaction that
    // reads or writes outside of its preprocessed dependencies.
    min_validation_idx: AtomicUsize,
    /// The number of validated transactions
    num_validated: AtomicUsize,
    /// The list of dependent transactions to resume when the
    /// key transaction is re-executed.
    transactions_dependents: Vec<Mutex<Vec<TxIdx>>>,
    /// A list of optional dependencies flagged during preprocessing.
    /// For instance, for a transaction to depend on two lower others,
    /// one send to the same recipient address, and one is from
    /// the same sender. We cannot casually check if all dependencies
    /// are clear with the dependents map as it can only lock the
    /// dependency. Two dependencies may check at the same time
    /// before they clear and think that the dependent is not yet
    /// ready, making it forever unexecuted.
    // TODO: Build a fuller dependency graph.
    transactions_dependencies: AHashMap<TxIdx, Mutex<Vec<TxIdx>>>,
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
            execution_idx: CachePadded::new(AtomicUsize::new(0)),
            transactions_status: transactions_status
                .into_iter()
                .map(|status| CachePadded::new(Mutex::new(status)))
                .collect(),
            transactions_dependents: transactions_dependents
                .into_iter()
                .map(Mutex::new)
                .collect(),
            transactions_dependencies: transactions_dependencies
                .into_iter()
                .map(|(tx_idx, deps)| (tx_idx, Mutex::new(deps)))
                .collect(),
            // We won't validate until we find the first transaction that
            // reads or writes outside of its preprocessed dependencies.
            validation_idx: CachePadded::new(AtomicUsize::new(block_size)),
            min_validation_idx: AtomicUsize::new(block_size),
            num_validated: AtomicUsize::new(0),
        }
    }

    pub(crate) fn done(&self) -> bool {
        self.execution_idx.load(Relaxed) >= self.block_size
            && self.validation_idx.load(Relaxed) >= self.block_size
            && self.num_validated.load(Relaxed)
                >= self.block_size - self.min_validation_idx.load(Relaxed)
    }

    fn try_incarnate(&self, mut tx_idx: TxIdx) -> Option<TxVersion> {
        while tx_idx < self.block_size {
            let mut tx = index_mutex!(self.transactions_status, tx_idx);
            if tx.status == IncarnationStatus::ReadyToExecute {
                tx.status = IncarnationStatus::Executing;
                return Some(TxVersion {
                    tx_idx,
                    tx_incarnation: tx.incarnation,
                });
            }
            drop(tx);
            tx_idx = self.execution_idx.fetch_add(1, Relaxed);
        }
        None
    }

    fn next_version_to_validate(&self) -> Option<TxVersion> {
        let mut validation_idx = self.validation_idx.fetch_add(1, Relaxed);
        while validation_idx < self.block_size {
            let tx = index_mutex!(self.transactions_status, validation_idx);
            if matches!(
                tx.status,
                IncarnationStatus::Executed | IncarnationStatus::Validated
            ) {
                return Some(TxVersion {
                    tx_idx: validation_idx,
                    tx_incarnation: tx.incarnation,
                });
            }
            drop(tx);
            validation_idx = self.validation_idx.fetch_add(1, Relaxed);
        }
        None
    }

    pub(crate) fn next_task(&self) -> Option<Task> {
        while !self.done() {
            if self.validation_idx.load(Relaxed) < self.execution_idx.load(Relaxed) {
                if let Some(tx_version) = self.next_version_to_validate() {
                    return Some(Task::Validation(tx_version));
                }
            }
            if let Some(tx_version) = self.try_incarnate(self.execution_idx.fetch_add(1, Relaxed)) {
                return Some(Task::Execution(tx_version));
            }
        }
        None
    }

    // Add `tx_idx` as a dependent of `blocking_tx_idx` so `tx_idx` is
    // re-executed when the next `blocking_tx_idx` incarnation is executed.
    // Return `false` if we encounter a race condition when `blocking_tx_idx`
    // gets re-executed before the dependency can be added.
    // TODO: Better error handling, including asserting that both indices are in range.
    pub(crate) fn add_dependency(&self, tx_idx: TxIdx, blocking_tx_idx: TxIdx) -> bool {
        // This is an important lock to prevent a race condition where the blocking
        // transaction completes re-execution before this dependency can be added.
        let blocking_tx = index_mutex!(self.transactions_status, blocking_tx_idx);
        if matches!(
            blocking_tx.status,
            IncarnationStatus::Executed | IncarnationStatus::Validated
        ) {
            return false;
        }

        let mut tx = index_mutex!(self.transactions_status, tx_idx);
        if tx.status == IncarnationStatus::Executing {
            tx.status = IncarnationStatus::Aborting;
            drop(tx);

            // TODO: Better error handling here
            let mut blocking_dependents =
                index_mutex!(self.transactions_dependents, blocking_tx_idx);
            blocking_dependents.push(tx_idx);
            drop(blocking_dependents);

            return true;
        }

        unreachable!("Trying to abort & add dependency in non-executing state!")
    }

    // Be careful as this one is usually called as a sub-routine that is very
    // easy to dead-lock.
    fn set_ready_status(&self, tx_idx: TxIdx) {
        // TODO: Better error handling
        let mut tx = index_mutex!(self.transactions_status, tx_idx);
        if tx.status == IncarnationStatus::Aborting {
            tx.status = IncarnationStatus::ReadyToExecute;
            tx.incarnation += 1;
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
        next_validation_idx: Option<TxIdx>,
    ) -> Option<ValidationTask> {
        // TODO: Better error handling
        let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
        if tx.status == IncarnationStatus::Executing {
            // TODO: Assert that `i` equals `tx_version.tx_incarnation`?
            tx.status = IncarnationStatus::Executed;
            drop(tx);

            // Resume dependent transactions
            let mut dependents = index_mutex!(self.transactions_dependents, tx_version.tx_idx);
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
                self.execution_idx.fetch_min(min_idx, Relaxed);
            }

            // Decide where to validate from next
            let min_validation_idx = if let Some(tx_idx) = next_validation_idx {
                min(self.min_validation_idx.fetch_min(tx_idx, Relaxed), tx_idx)
            } else {
                self.min_validation_idx.load(Relaxed)
            };
            // Have found a min validation index to even bother
            if min_validation_idx < self.block_size {
                // Must re-validate from min as this transaction is lower
                if tx_version.tx_idx < min_validation_idx {
                    if wrote_new_location {
                        self.validation_idx.fetch_min(min_validation_idx, Relaxed);
                    }
                }
                // Validate from this transaction as it's in between min and the current
                // validation index.
                else if tx_version.tx_idx < self.validation_idx.load(Relaxed) {
                    if wrote_new_location {
                        self.validation_idx
                            .fetch_min(tx_version.tx_idx + 1, Relaxed);
                    }
                    return Some(tx_version);
                }
                // Don't need to validate anything if the current validation index is
                // lower or equal -- it will catch up later.
            }
        } else {
            // TODO: Better error handling
            unreachable!("Trying to finish execution in a non-executing state")
        }
        None
    }

    // Return whether the abort was successful. A successful abort leads to
    // scheduling the transaction for re-execution and the higher transactions
    // for validation during `finish_validation`. The scheduler ensures that only
    // one failing validation per version can lead to a successful abort.
    pub(crate) fn try_validation_abort(&self, tx_version: &TxVersion) -> bool {
        // TODO: Better error handling
        let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
        if tx.status == IncarnationStatus::Validated {
            self.num_validated.fetch_sub(1, Relaxed);
        }

        let aborting = matches!(
            tx.status,
            IncarnationStatus::Executed | IncarnationStatus::Validated
        );
        if aborting {
            tx.status = IncarnationStatus::Aborting;
        }
        aborting
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
            self.validation_idx
                .fetch_min(tx_version.tx_idx + 1, Relaxed);
            if self.execution_idx.load(Relaxed) > tx_version.tx_idx {
                return self.try_incarnate(tx_version.tx_idx);
            }
        } else {
            let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
            if tx.status == IncarnationStatus::Executed {
                tx.status = IncarnationStatus::Validated;
                self.num_validated.fetch_add(1, Relaxed);
            }
        }
        None
    }
}
