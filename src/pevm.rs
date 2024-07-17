use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    num::NonZeroUsize,
    sync::{Mutex, OnceLock},
    thread,
};

use alloy_chains::Chain;
use alloy_primitives::U256;
use alloy_rpc_types::{Block, BlockTransactions};
use defer_drop::DeferDrop;
use revm::{
    primitives::{
        AccountInfo, AccountStatus, BlockEnv, Bytecode, ResultAndState,
        SpecId::{self, SPURIOUS_DRAGON},
        TransactTo, TxEnv, KECCAK_EMPTY,
    },
    DatabaseCommit, DatabaseRef, StateBuilder,
};

use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    primitives::{get_block_env, get_block_spec, get_tx_env, TransactionParsingError},
    scheduler::Scheduler,
    vm::{build_evm, ExecutionError, Vm, VmExecutionResult},
    BuildIdentityHasher, MemoryEntry, MemoryLocation, MemoryValue, Task, TxIdx, TxVersion,
};

/// Errors when executing a block with PEVM.
#[derive(Debug, PartialEq, Clone)]
pub enum PevmError {
    /// Cannot derive the chain spec from the block header.
    UnknownBlockSpec,
    /// Block header lacks information for execution.
    MissingHeaderData,
    /// Transactions lack information for execution.
    MissingTransactionData,
    /// Invalid input transaction.
    InvalidTransaction(TransactionParsingError),
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
pub type PevmResult = Result<Vec<ResultAndState>, PevmError>;

// TODO: Add a [Pevm] struct for long-lasting use to minimize
// (de)allocations between runs.

/// Execute an Alloy block, which is becoming the "standard" format in Rust.
/// TODO: Better error handling.
pub fn execute<DB: DatabaseRef<Error: Display> + Send + Sync>(
    db: &DB,
    chain: Chain,
    block: Block,
    concurrency_level: NonZeroUsize,
    force_sequential: bool,
) -> PevmResult {
    let Some(spec_id) = get_block_spec(&block.header) else {
        return Err(PevmError::UnknownBlockSpec);
    };
    let Some(block_env) = get_block_env(&block.header) else {
        return Err(PevmError::MissingHeaderData);
    };
    let tx_envs = match block.transactions {
        BlockTransactions::Full(txs) => txs
            .into_iter()
            .map(get_tx_env)
            .collect::<Result<Vec<TxEnv>, TransactionParsingError>>()
            .map_err(PevmError::InvalidTransaction)?,
        _ => return Err(PevmError::MissingTransactionData),
    };
    // TODO: Continue to fine tune this condition.
    if force_sequential || tx_envs.len() < 4 || block.header.gas_used < 2_000_000 {
        execute_revm_sequential(db, chain, spec_id, block_env, tx_envs)
    } else {
        execute_revm(db, chain, spec_id, block_env, tx_envs, concurrency_level)
    }
}

/// Execute an REVM block.
// Ideally everyone would go through the [Alloy] interface. This one is currently
// useful for testing, and for users that are heavily tied to Revm like Reth.
pub fn execute_revm<DB: DatabaseRef<Error: Display> + Send + Sync>(
    db: &DB,
    chain: Chain,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
    concurrency_level: NonZeroUsize,
) -> PevmResult {
    if txs.is_empty() {
        return Ok(Vec::new());
    }

    // Preprocess locations
    let block_size = txs.len();
    let hasher = ahash::RandomState::new();
    let beneficiary_location_hash = hasher.hash_one(MemoryLocation::Basic(block_env.coinbase));
    // TODO: Estimate more locations based on sender, to, etc.
    let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
    estimated_locations.insert(
        beneficiary_location_hash,
        (0..block_size).collect::<Vec<TxIdx>>(),
    );
    let mut lazy_addresses = LazyAddresses::default();
    lazy_addresses.0.insert(block_env.coinbase);

    // Initialize the remaining core components
    // TODO: Provide more explicit garbage collecting configs for users over random background
    // threads like this. For instance, to have a dedicated thread (pool) for cleanup.
    let mv_memory = DeferDrop::new(MvMemory::new(
        block_size,
        estimated_locations,
        lazy_addresses,
    ));
    let txs = DeferDrop::new(txs);
    let vm = Vm::new(&hasher, db, &mv_memory, &txs, chain, spec_id, block_env);
    let scheduler = DeferDrop::new(Scheduler::new(block_size));

    let mut execution_error = OnceLock::new();
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
                            &execution_error,
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
                    // the default behaviour for now. Also, be aware of a potential deadlock
                    // in the scheduler's next task loop when an error occurs.
                    if execution_error.get().is_some() {
                        break;
                    }

                    if task.is_none() {
                        task = scheduler.next_task();
                    }
                }
            });
        }
    });

    if let Some(err) = execution_error.take() {
        return Err(PevmError::ExecutionError(format!("{err:?}")));
    }

    let mut fully_evaluated_results = Vec::with_capacity(block_size);
    for mutex in execution_results {
        fully_evaluated_results.push(mutex.into_inner().unwrap().unwrap());
    }

    // We fully evaluate (the balance and nonce of) the beneficiary account
    // and raw transfer recipients that may have been atomically updated.
    for address in mv_memory.consume_lazy_addresses() {
        let location_hash = hasher.hash_one(MemoryLocation::Basic(address));
        if let Some(write_history) = mv_memory.consume_location(&location_hash) {
            // TODO: We don't need to read from storage if the first entry is a fully evaluated account.
            let mut is_first = false;
            let mut current_account = match db.basic_ref(address) {
                Ok(Some(account)) => account,
                _ => {
                    is_first = true;
                    AccountInfo::default()
                }
            };

            // TODO: Assert that the evaluated nonce matches the tx's.
            for (tx_idx, memory_entry) in write_history {
                let mut self_destructed = false;
                let tx = unsafe { txs.get_unchecked(tx_idx) };
                match memory_entry {
                    MemoryEntry::Data(_, MemoryValue::Basic(info)) => {
                        if let Some(info) = info {
                            current_account.balance = info.balance;
                            current_account.nonce = info.nonce;
                        } else {
                            // TODO: Be careful of contracts re-deployed in the same block
                            // that it is self-destructed, especially if there is an inbetween
                            // transaction that funds it (to trigger lazy evaluation).
                            self_destructed = true;
                        }
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
                if is_first {
                    account.status = AccountStatus::LoadedAsNotExisting;
                    if current_account.is_empty_code_hash() && !account.info.is_empty_code_hash()
                        || tx.transact_to == TransactTo::Create
                            && location_hash != beneficiary_location_hash
                    {
                        account.status |= AccountStatus::Created;
                        if account.info.code.is_none() {
                            account.info.code = Some(Bytecode::new());
                        }
                    }
                    is_first = false;
                } else {
                    account.status -= AccountStatus::LoadedAsNotExisting;
                }
                if self_destructed {
                    current_account.balance = U256::ZERO;
                    current_account.nonce = 0;
                    current_account.code_hash = KECCAK_EMPTY;
                } else {
                    account.info.balance = current_account.balance;
                    account.info.nonce = current_account.nonce;
                    if !current_account.is_empty_code_hash() {
                        account.info.code_hash = current_account.code_hash;
                        account.info.code.clone_from(&current_account.code);
                    }
                }
                account.mark_touch();
                if self_destructed || spec_id.is_enabled_in(SPURIOUS_DRAGON) && account.is_empty() {
                    is_first = true;
                }
            }
        }
    }

    Ok(fully_evaluated_results)
}

/// Execute REVM transactions sequentially.
// Useful for falling back for (small) blocks with many dependencies.
// TODO: Use this for a long chain of sequential transactions even in parallel mode.
pub fn execute_revm_sequential<DB: DatabaseRef<Error: Display>>(
    db: &DB,
    chain: Chain,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) -> Result<Vec<ResultAndState>, PevmError> {
    // We use [State] with this specific builder instead of [CacheDB] to match Reth
    // as close as possible.
    let mut builder = StateBuilder::new_with_database(db);
    if !spec_id.is_enabled_in(SPURIOUS_DRAGON) {
        builder = builder.without_state_clear();
    }
    let mut db = builder.build();
    let mut evm = build_evm(&mut db, chain, spec_id, block_env, true);
    let mut results = Vec::with_capacity(txs.len());
    for tx in txs {
        *evm.tx_mut() = tx;
        match evm.transact() {
            Ok(result_and_state) => {
                evm.db_mut().commit(result_and_state.state.clone());
                results.push(result_and_state);
            }
            Err(err) => return Err(PevmError::ExecutionError(err.to_string())),
        }
    }
    Ok(results)
}

fn try_execute<DB: DatabaseRef<Error: Display>>(
    mv_memory: &MvMemory,
    vm: &Vm<DB>,
    scheduler: &Scheduler,
    execution_error: &OnceLock<ExecutionError>,
    execution_results: &[Mutex<Option<ResultAndState>>],
    tx_version: TxVersion,
) -> Option<Task> {
    loop {
        return match vm.execute(tx_version.tx_idx, true) {
            VmExecutionResult::Retry => continue,
            VmExecutionResult::ReadError { blocking_tx_idx } => {
                if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx) {
                    // Retry the execution immediately if the blocking transaction was
                    // re-executed by the time we can add it as a dependency.
                    continue;
                }
                None
            }
            VmExecutionResult::ExecutionError(err) => {
                // TODO: Better error handling
                execution_error.set(err).unwrap();
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
