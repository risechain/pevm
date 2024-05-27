use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use ahash::{AHashMap, AHashSet};
use alloy_primitives::{Address, U256};
use alloy_rpc_types::{Block, Header};
use revm::primitives::{Account, AccountInfo, BlockEnv, ResultAndState, SpecId, TransactTo, TxEnv};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    primitives::{get_block_env, get_block_spec, get_tx_envs},
    scheduler::Scheduler,
    vm::{ExecutionError, Vm, VmExecutionResult},
    ExecutionTask, MemoryLocation, MemoryValue, Storage, Task, TransactionsDependencies,
    TransactionsDependents, TransactionsStatus, TxIdx, TxIncarnationStatus, TxVersion,
    ValidationTask,
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
    /// Impractical errors that should be unreachable.
    /// The library has bugs if this is yielded.
    UnreachableError,
}

/// Execution result of BlockSTM.
pub type BlockStmResult = Result<Vec<ResultAndState>, BlockStmError>;

/// Execute an Alloy block, which is becoming the "standard" format in Rust.
/// TODO: Better error handling.
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
// TODO: Better error handling.
pub fn execute_revm<S: Storage + Send + Sync>(
    storage: S,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
    concurrency_level: NonZeroUsize,
) -> BlockStmResult {
    // Beneficiary setup for post-processing
    let beneficiary_address = block_env.coinbase;
    let beneficiary_location = MemoryLocation::Basic(beneficiary_address);
    let mut beneficiary_account_info = match storage.basic(beneficiary_address) {
        Ok(Some(account)) => account.into(),
        _ => AccountInfo::default(),
    };

    // Initialize main components
    let block_size = txs.len();
    let (
        transactions_status,
        transactions_dependents,
        transactions_dependencies,
        max_concurrency_level,
    ) = preprocess_dependencies(&beneficiary_address, &txs);
    let scheduler = Scheduler::new(
        block_size,
        transactions_status,
        transactions_dependents,
        transactions_dependencies,
    );
    let mv_memory = Arc::new(MvMemory::new(block_size));
    let vm = Vm::new(spec_id, block_env, txs, storage, mv_memory.clone());

    // Edge case that is pretty common
    // TODO: Shortcut even before initializing the main parallel components.
    // It would require structuring a cleaner trait interface for any Storage
    // to act as the standalone DB for sequential execution.
    if block_size == 1 {
        return match vm.execute(0) {
            VmExecutionResult::ExecutionError(err) => Err(BlockStmError::ExecutionError(err)),
            VmExecutionResult::Ok {
                mut result_and_state,
                write_set,
                ..
            } => {
                for (location, value) in write_set {
                    if location == beneficiary_location {
                        result_and_state.state.insert(
                            beneficiary_address,
                            post_process_beneficiary(&mut beneficiary_account_info, value),
                        );
                        break;
                    }
                }
                Ok(vec![result_and_state])
            }
            _ => Err(BlockStmError::UnreachableError),
        };
    }

    // Start multithreading mainline
    let mut execution_error = OnceLock::new();
    let execution_results = (0..block_size).map(|_| Mutex::new(None)).collect();

    // TODO: Better thread handling
    thread::scope(|scope| {
        for _ in 0..concurrency_level.min(max_concurrency_level).into() {
            scope.spawn(|| {
                let mut task = None;
                let mut consecutive_empty_tasks: u8 = 0;
                while !scheduler.done() {
                    // TODO: Have different functions or an enum for the caller to choose
                    // the handling behaviour when a transaction's EVM execution fails.
                    // Parallel block builders would like to exclude such transaction,
                    // verifiers may want to exit early to save CPU cycles, while testers
                    // may want to collect all execution results. We are exiting early as
                    // the default behaviour for now.
                    if execution_error.get().is_some() {
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
                            tx_version,
                        )
                        .map(Task::Validation),
                        Some(Task::Validation(tx_version)) => {
                            try_validate(&mv_memory, &scheduler, &tx_version).map(Task::Execution)
                        }
                        None => scheduler.next_task(),
                    };

                    if task.is_none() {
                        consecutive_empty_tasks += 1;
                    } else {
                        consecutive_empty_tasks = 0;
                    }
                    // Many consecutive empty tasks usually mean the number of remaining tasks
                    // is smaller than the number of threads, or they are highly sequential
                    // anyway. This early exit helps remove thread overheads and join faster.
                    if consecutive_empty_tasks == 3 {
                        break;
                    }
                }
            });
        }
    });

    if let Some(err) = execution_error.take() {
        return Err(BlockStmError::ExecutionError(err));
    }

    // We lazily evaluate the final beneficiary account's balance at the end of each transaction
    // to avoid "implicit" dependency among consecutive transactions that read & write there.
    // TODO: Refactor, improve speed & error handling.
    Ok(execution_results
        .iter()
        .map(|m| m.lock().unwrap().take().unwrap())
        .enumerate()
        .map(|(tx_idx, mut result_and_state)| {
            match mv_memory.read_absolute(&beneficiary_location, tx_idx) {
                ReadMemoryResult::Ok { value, .. } => {
                    result_and_state.state.insert(
                        beneficiary_address,
                        post_process_beneficiary(&mut beneficiary_account_info, value),
                    );
                    result_and_state
                }
                _ => unreachable!(),
            }
        })
        .collect())
}

// TODO: Make this as fast as possible.
fn preprocess_dependencies(
    beneficiary_address: &Address,
    txs: &[TxEnv],
) -> (
    TransactionsStatus,
    TransactionsDependents,
    TransactionsDependencies,
    NonZeroUsize,
) {
    let block_size = txs.len();
    if block_size == 0 {
        return (Vec::new(), Vec::new(), AHashMap::new(), NonZeroUsize::MIN);
    }

    let mut transactions_status: TransactionsStatus = (0..block_size)
        .map(|_| TxIncarnationStatus::ReadyToExecute(0))
        .collect();
    let mut transactions_dependents: TransactionsDependents =
        (0..block_size).map(|_| AHashSet::new()).collect();
    let mut transactions_dependencies: TransactionsDependencies = AHashMap::new();

    // Marking transactions from the same sender as dependents (all write to nonce).
    let mut tx_idxes_by_sender: AHashMap<Address, Vec<TxIdx>> = AHashMap::new();
    // Marking transactions to the same address as dependents.
    let mut tx_idxes_by_recipients: AHashMap<Address, Vec<TxIdx>> = AHashMap::new();
    for (tx_idx, tx) in txs.iter().enumerate() {
        let mut register_dependency = |dependency_idx: usize| {
            // SAFETY: The dependency index is guaranteed to be smaller than the block
            // size in this scope.
            unsafe {
                *transactions_status.get_unchecked_mut(tx_idx) = TxIncarnationStatus::Aborting(0);
                transactions_dependents
                    .get_unchecked_mut(dependency_idx)
                    .insert(tx_idx);
            }
            transactions_dependencies
                .entry(tx_idx)
                .or_default()
                .insert(dependency_idx);
        };

        if &tx.caller == beneficiary_address && tx_idx > 0 {
            register_dependency(tx_idx - 1);
        }
        // Sender as Sender
        if let Some(prev_idx) = tx_idxes_by_sender
            .get(&tx.caller)
            .and_then(|tx_idxs| tx_idxs.last())
        {
            register_dependency(*prev_idx);
        }
        // Sender as Recipient
        // This is critical to avoid this nasty race condition:
        // 1. A spends money
        // 2. B sends money to A
        // 3. A spends money
        // Without (3) depending on (2), (2) may race and write to A first, then (1) comes
        // second flagging (2) for re-execution and execute (3) as dependency. (3) would
        // panic with a nonce error reading from (2) before it rewrites the new nonce
        // reading from (1).
        if let Some(prev_idx) = tx_idxes_by_recipients
            .get(&tx.caller)
            .and_then(|tx_idxs| tx_idxs.last())
        {
            register_dependency(*prev_idx);
        }
        // We check for a non-empty value that guarantees to update the balance of the recipient,
        // to avoid smart contract interactions that only change some storage slots, etc.
        if tx.value != U256::ZERO {
            if let TransactTo::Call(to) = tx.transact_to {
                if &to == beneficiary_address && tx_idx > 0 {
                    register_dependency(tx_idx - 1);
                }
                // Recipient as Sender
                if let Some(prev_idx) = tx_idxes_by_sender
                    .get(&to)
                    .and_then(|tx_idxs| tx_idxs.last())
                {
                    register_dependency(*prev_idx);
                }
                // Recipient as Recipient
                let recipient_tx_idxes = tx_idxes_by_recipients.entry(to).or_default();
                if let Some(prev_idx) = recipient_tx_idxes.last() {
                    register_dependency(*prev_idx);
                }
                recipient_tx_idxes.push(tx_idx);
            }
        }
        tx_idxes_by_sender
            .entry(tx.caller)
            .or_default()
            .push(tx_idx);
    }

    let min_concurrency_level = NonZeroUsize::new(2).unwrap();
    // This division by 2 means a thread must complete ~4 tasks to justify
    // its overheads.
    // TODO: Fine tune for edge cases given the dependency data above.
    let max_concurrency_level = NonZeroUsize::new(block_size / 2)
        .unwrap_or(min_concurrency_level)
        .max(min_concurrency_level);

    (
        transactions_status,
        transactions_dependents,
        transactions_dependencies,
        max_concurrency_level,
    )
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
    execution_error: &OnceLock<ExecutionError>,
    execution_results: &Vec<Mutex<Option<ResultAndState>>>,
    tx_version: TxVersion,
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
            // TODO: Better error handling
            execution_error.set(err).unwrap();
            None
        }
        VmExecutionResult::Ok {
            result_and_state,
            read_set,
            write_set,
        } => {
            *index_mutex!(execution_results, tx_version.tx_idx) = Some(result_and_state);
            let wrote_new_location = mv_memory.record(&tx_version, read_set, write_set);
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

// Fully evaluate a beneficiary account at the end of block execution,
// including lazy updating atomic balances.
// TODO: Cleaner interface & error handling
fn post_process_beneficiary(
    beneficiary_account_info: &mut AccountInfo,
    value: MemoryValue,
) -> Account {
    match value {
        MemoryValue::Basic(info) => {
            *beneficiary_account_info = *info;
        }
        MemoryValue::LazyBeneficiaryBalance(addition) => {
            beneficiary_account_info.balance += addition;
        }
        _ => unreachable!(),
    }
    // TODO: This potentially wipes beneficiary account's storage.
    // Does that happen and if so is it acceptable? A quick test with
    // REVM wipes it too!
    let mut beneficiary_account = Account::from(beneficiary_account_info.clone());
    beneficiary_account.mark_touch();
    beneficiary_account
}
