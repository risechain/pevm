use std::{
    cmp::min,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    thread,
};

use smallvec::SmallVec;

use crate::{FinishExecFlags, IncarnationStatus, Task, TxIdx, TxStatus, TxVersion};

// The Pevm collaborative scheduler coordinates execution & validation
// tasks among work threads.
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
#[derive(Debug)]
pub(crate) struct Scheduler {
    // The number of transactions in this block.
    block_size: usize,
    // The most up-to-date incarnation number (initially 0) and
    // the status of this incarnation.
    // TODO: Consider packing [TxStatus]s into atomics instead of
    // [Mutex] given how small they are.
    transactions_status: Vec<Mutex<TxStatus>>,
    // The list of dependent transactions to resume when the
    // key transaction is re-executed.
    transactions_dependents: Vec<Mutex<SmallVec<[TxIdx; 1]>>>,
    // The next transaction to try and execute.
    execution_idx: AtomicUsize,
    // The next transaction to try and validate.
    validation_idx: AtomicUsize,
    // We won't validate until we find the first non-lazy transaction that
    // needs to read explicit values. We also skip the first transaction.
    min_validation_idx: AtomicUsize,
    // The number of validated transactions
    num_validated: AtomicUsize,
    // True if the scheduler has been aborted, likely due to fatal execution
    // errors.
    aborted: AtomicBool,
}

// TODO: Better error handling.
// Like returning errors instead of panicking on [unreachable]s.
impl Scheduler {
    pub(crate) fn new(block_size: usize) -> Self {
        Self {
            block_size,
            execution_idx: AtomicUsize::new(0),
            transactions_status: (0..block_size)
                .map(|_| {
                    Mutex::new(TxStatus {
                        incarnation: 0,
                        status: IncarnationStatus::ReadyToExecute,
                    })
                })
                .collect(),
            transactions_dependents: (0..block_size).map(|_| Mutex::default()).collect(),
            // We won't validate until we find the first non-lazy transaction that
            // needs to read explicit values. We also skip the first transaction.
            validation_idx: AtomicUsize::new(block_size),
            min_validation_idx: AtomicUsize::new(block_size),
            num_validated: AtomicUsize::new(0),
            aborted: AtomicBool::new(false),
        }
    }

    pub(crate) fn abort(&self) {
        self.aborted.store(true, Ordering::Relaxed);
    }

    fn try_execute(&self, tx_idx: TxIdx) -> Option<TxVersion> {
        if tx_idx < self.block_size {
            let mut tx = index_mutex!(self.transactions_status, tx_idx);
            if tx.status == IncarnationStatus::ReadyToExecute {
                tx.status = IncarnationStatus::Executing;
                return Some(TxVersion {
                    tx_idx,
                    tx_incarnation: tx.incarnation,
                });
            }
        }
        None
    }

    pub(crate) fn next_task(&self) -> Option<Task> {
        while !self.aborted.load(Ordering::Relaxed) {
            let execution_idx = self.execution_idx.load(Ordering::Relaxed);
            let validation_idx = self.validation_idx.load(Ordering::Relaxed);
            if execution_idx >= self.block_size && validation_idx >= self.block_size {
                if self.num_validated.load(Ordering::Relaxed)
                    >= self.block_size - self.min_validation_idx.load(Ordering::Relaxed)
                {
                    break;
                }
                thread::yield_now();
                continue;
            }

            // Prioritize a validation task to minimize re-execution
            if validation_idx < execution_idx {
                let tx_idx = self.validation_idx.fetch_add(1, Ordering::Relaxed);
                if tx_idx < self.block_size {
                    let mut tx = index_mutex!(self.transactions_status, tx_idx);
                    // "Steal" execution job while holding the lock
                    if tx.status == IncarnationStatus::ReadyToExecute {
                        tx.status = IncarnationStatus::Executing;
                        return Some(Task::Execution(TxVersion {
                            tx_idx,
                            tx_incarnation: tx.incarnation,
                        }));
                    }
                    // Start a typical validation task
                    if matches!(
                        tx.status,
                        IncarnationStatus::Executed | IncarnationStatus::Validated
                    ) {
                        return Some(Task::Validation(TxVersion {
                            tx_idx,
                            tx_incarnation: tx.incarnation,
                        }));
                    }
                    // Validation index is still catching up so continue a
                    // new loop iteration to refetch the latest indices
                    // before deciding again.
                    if tx.status == IncarnationStatus::Aborting {
                        continue;
                    }
                    // Fall back to execution job as this executing tx will
                    // decide if validation is needed when it's done. If it
                    // does, all validation tasks here would be redone anyway.
                }
            }

            // Prioritize execution task
            if let Some(tx_version) =
                self.try_execute(self.execution_idx.fetch_add(1, Ordering::Relaxed))
            {
                return Some(Task::Execution(tx_version));
            }
        }
        None
    }

    // Add [tx_idx] as a dependent of [blocking_tx_idx] so [tx_idx] is
    // re-executed when the next [blocking_tx_idx] incarnation is executed.
    // Return [false] if we encounter a race condition when [blocking_tx_idx]
    // gets re-executed before the dependency can be added.
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
        debug_assert_eq!(tx.status, IncarnationStatus::Executing);
        tx.status = IncarnationStatus::Aborting;

        let mut blocking_dependents = index_mutex!(self.transactions_dependents, blocking_tx_idx);
        blocking_dependents.push(tx_idx);

        true
    }

    fn set_ready_status(&self, tx_idx: TxIdx) {
        let mut tx = index_mutex!(self.transactions_status, tx_idx);
        debug_assert_eq!(tx.status, IncarnationStatus::Aborting);
        tx.status = IncarnationStatus::ReadyToExecute;
        tx.incarnation += 1;
    }

    pub(crate) fn finish_execution(
        &self,
        tx_version: TxVersion,
        flags: FinishExecFlags,
    ) -> Option<Task> {
        let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
        debug_assert_eq!(tx.status, IncarnationStatus::Executing);
        debug_assert_eq!(tx.incarnation, tx_version.tx_incarnation);

        // Resume dependent transactions
        let mut dependents = index_mutex!(self.transactions_dependents, tx_version.tx_idx);
        for tx_idx in dependents.drain(..) {
            self.set_ready_status(tx_idx);
            self.execution_idx.fetch_min(tx_idx, Ordering::Relaxed);
        }

        // TODO: Simplify or better document this logic.
        // Decide where to validate from next
        let min_validation_idx = if flags.contains(FinishExecFlags::NeedValidation) {
            min(
                self.min_validation_idx
                    .fetch_min(tx_version.tx_idx, Ordering::Relaxed),
                tx_version.tx_idx,
            )
        } else {
            self.min_validation_idx.load(Ordering::Relaxed)
        };
        // Have found a min validation index to even bother
        if min_validation_idx < self.block_size {
            // Must re-validate from min as this transaction is lower
            if tx_version.tx_idx < min_validation_idx {
                if flags.contains(FinishExecFlags::WroteNewLocation) {
                    self.validation_idx
                        .fetch_min(min_validation_idx, Ordering::Relaxed);
                }
            }
            // Validate from this transaction as it's in between min and the current
            // validation index.
            else if tx_version.tx_idx < self.validation_idx.load(Ordering::Relaxed) {
                if flags.contains(FinishExecFlags::WroteNewLocation) {
                    self.validation_idx
                        .fetch_min(tx_version.tx_idx + 1, Ordering::Relaxed);
                }
                if flags.contains(FinishExecFlags::NeedValidation) {
                    tx.status = IncarnationStatus::Executed;
                    return Some(Task::Validation(tx_version));
                }
                tx.status = IncarnationStatus::Validated;
                self.num_validated.fetch_add(1, Ordering::Relaxed);
            }
            // Don't need to validate anything if the current validation index is
            // lower or equal -- it will catch up later.
        }

        if flags.contains(FinishExecFlags::NeedValidation) {
            tx.status = IncarnationStatus::Executed;
        } else {
            tx.status = IncarnationStatus::Validated;
            self.num_validated.fetch_add(1, Ordering::Relaxed);
        }
        None
    }

    // Return whether the abort was successful. A successful abort leads to
    // scheduling the transaction for re-execution and the higher transactions
    // for validation during [finish_validation]. The scheduler ensures that only
    // one failing validation per version can lead to a successful abort.
    pub(crate) fn try_validation_abort(&self, tx_version: &TxVersion) -> bool {
        let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
        if tx.status == IncarnationStatus::Validated {
            self.num_validated.fetch_sub(1, Ordering::Relaxed);
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
    pub(crate) fn finish_validation(&self, tx_version: &TxVersion, aborted: bool) -> Option<Task> {
        if aborted {
            self.set_ready_status(tx_version.tx_idx);
            self.validation_idx
                .fetch_min(tx_version.tx_idx + 1, Ordering::Relaxed);
            if self.execution_idx.load(Ordering::Relaxed) > tx_version.tx_idx {
                return self.try_execute(tx_version.tx_idx).map(Task::Execution);
            }
        } else {
            let mut tx = index_mutex!(self.transactions_status, tx_version.tx_idx);
            if tx.status == IncarnationStatus::Executed {
                tx.status = IncarnationStatus::Validated;
                self.num_validated.fetch_add(1, Ordering::Relaxed);
            }
        }
        None
    }
}
