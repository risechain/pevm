use std::{
    cell::UnsafeCell,
    fmt::Debug,
    num::NonZeroUsize,
    sync::{OnceLock, mpsc},
    thread,
};

use alloy_primitives::{TxNonce, U256};
use rayon::prelude::*;
use alloy_rpc_types_eth::{Block, BlockTransactions};
use hashbrown::HashMap;
use revm::{
    DatabaseCommit, ExecuteEvm,
    context::{BlockEnv, ContextTr, Transaction, result::InvalidTransaction},
    database::CacheDB,
    handler::EvmTr,
};

use crate::{
    EvmAccount, MemoryEntry, MemoryLocation, MemoryValue, Storage, Task, TxIdx, TxVersion,
    chain::PevmChain,
    compat::get_block_env,
    hash_deterministic,
    mv_memory::{MvEntries, MvMemory},
    scheduler::Scheduler,
    storage::StorageWrapper,
    vm::{ExecutionError, PevmTxExecutionResult, Vm, VmExecutionError, VmExecutionResult},
};

/// Errors when executing a block with pevm.
// TODO: implement traits explicitly due to trait bounds on `C` instead of types of `PevmChain`
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PevmError<C: PevmChain> {
    /// Cannot derive the chain spec from the block header.
    #[error("Cannot derive the chain spec from the block header")]
    BlockSpecError(#[source] C::BlockSpecError),
    /// Transactions lack information for execution.
    #[error("Transactions lack information for execution")]
    MissingTransactionData,
    /// Invalid input transaction.
    #[error("Invalid input transaction")]
    InvalidTransaction(#[source] C::TransactionParsingError),
    /// Nonce too low or too high
    #[error("Nonce mismatch for tx #{tx_idx}. Expected {executed_nonce}, got {tx_nonce}")]
    NonceMismatch {
        /// Transaction index
        tx_idx: TxIdx,
        /// Nonce from tx (from the very input)
        tx_nonce: TxNonce,
        /// Nonce from state and execution
        executed_nonce: TxNonce,
    },
    /// Storage error.
    // TODO: More concrete types than just an arbitrary string.
    #[error("Storage error: {0}")]
    StorageError(String),
    /// EVM execution error.
    #[error("Execution error")]
    ExecutionError(
        #[source]
        #[from]
        ExecutionError,
    ),
    /// Impractical errors that should be unreachable.
    /// The library has bugs if this is yielded.
    #[error(
        "PEVM encountered a bug. Please open an issue in https://github.com/risechain/pevm/issues/new"
    )]
    UnreachableError,
}

/// Execution result of a block
pub type PevmResult<C> = Result<Vec<PevmTxExecutionResult>, PevmError<C>>;

#[derive(Debug)]
enum AbortReason {
    FallbackToSequential,
    ExecutionError(ExecutionError),
}

// TODO: Better implementation
#[derive(Debug)]
struct AsyncDropper<T> {
    sender: mpsc::Sender<T>,
    _handle: thread::JoinHandle<()>,
}

impl<T: Send + 'static> Default for AsyncDropper<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            _handle: std::thread::spawn(move || receiver.into_iter().for_each(drop)),
        }
    }
}

impl<T> AsyncDropper<T> {
    fn drop(&self, t: T) {
        let _ = self.sender.send(t);
    }
}

// Per-tx slot for execution results. UnsafeCell allows worker threads to write through
// a shared reference without taking a lock; the Block-STM scheduler guarantees at most
// one writer per tx_idx at a time, so writes to different slots never race.
#[derive(Debug, Default)]
struct ExecutionResultSlot(UnsafeCell<Option<PevmTxExecutionResult>>);

// SAFETY: see struct-level invariant.
unsafe impl Sync for ExecutionResultSlot {}

impl ExecutionResultSlot {
    fn write(&self, value: PevmTxExecutionResult) {
        // SAFETY: see struct-level invariant.
        unsafe { *self.0.get() = Some(value) };
    }

    fn take(&mut self) -> Option<PevmTxExecutionResult> {
        self.0.get_mut().take()
    }
}

// TODO: Port more recyclable resources into here.
#[derive(Debug, Default)]
/// The main pevm struct that executes blocks.
pub struct Pevm {
    execution_results: Vec<ExecutionResultSlot>,
    abort_reason: OnceLock<AbortReason>,
    dropper: AsyncDropper<(MvMemory, Scheduler)>,
}

impl Pevm {
    /// Execute an Alloy block, which is becoming the "standard" format in Rust.
    /// TODO: Better error handling.
    pub fn execute<S, C>(
        &mut self,
        chain: &C,
        storage: &S,
        // We assume the block is still needed afterwards like in most Reth cases
        // so take in a reference and only copy values when needed. We may want
        // to use a [`std::borrow::Cow`] to build [`BlockEnv`] and [`TxEnv`] without
        // (much) copying when ownership can be given. Another challenge with this is
        // the new Alloy [`Transaction`] interface that is mostly `&self`. We'd need
        // to do some dirty destruction to get the owned fields.
        block: &Block<C::Transaction>,
        concurrency_level: NonZeroUsize,
        force_sequential: bool,
    ) -> PevmResult<C>
    where
        C: PevmChain + Send + Sync,
        S: Storage + Send + Sync + Debug,
    {
        let spec_id = chain
            .get_block_spec(&block.header)
            .map_err(PevmError::BlockSpecError)?;
        let block_env = get_block_env(&block.header, spec_id);
        let tx_envs = match &block.transactions {
            BlockTransactions::Full(txs) => txs
                .iter()
                .map(|tx| chain.get_tx_env(tx))
                .collect::<Result<Vec<_>, _>>()
                .map_err(PevmError::InvalidTransaction)?,
            _ => return Err(PevmError::MissingTransactionData),
        };
        // TODO: Continue to fine tune this condition.
        if force_sequential
            || tx_envs.len() < concurrency_level.into()
            || block.header.gas_used < 4_000_000
        {
            execute_revm_sequential(chain, storage, spec_id, block_env, tx_envs)
        } else {
            self.execute_revm_parallel(
                chain,
                storage,
                spec_id,
                block_env,
                tx_envs,
                concurrency_level,
            )
        }
    }

    /// Execute an REVM block.
    // Ideally everyone would go through the [Alloy] interface. This one is currently
    // useful for testing, and for users that are heavily tied to Revm like Reth.
    pub fn execute_revm_parallel<S, C>(
        &mut self,
        chain: &C,
        storage: &S,
        spec_id: C::EvmSpecId,
        block_env: BlockEnv,
        txs: Vec<C::EvmTx>,
        concurrency_level: NonZeroUsize,
    ) -> PevmResult<C>
    where
        C: PevmChain + Send + Sync,
        S: Storage + Send + Sync + Debug,
    {
        if txs.is_empty() {
            return Ok(Vec::new());
        }

        let block_size = txs.len();
        let scheduler = Scheduler::new(block_size);

        let mv_memory = chain.build_mv_memory(&block_env, &txs);

        let additional = block_size.saturating_sub(self.execution_results.len());
        if additional > 0 {
            self.execution_results.reserve(additional);
            for _ in 0..additional {
                self.execution_results.push(ExecutionResultSlot::default());
            }
        }

        // TODO: Better thread handling
        thread::scope(|scope| {
            for _ in 0..concurrency_level.into() {
                scope.spawn(|| {
                    let mut vm = Vm::new(chain, spec_id, &block_env, &txs, storage, &mv_memory);
                    let mut task = scheduler.next_task();
                    while task.is_some() {
                        task = match task.unwrap() {
                            Task::Execution(tx_version) => {
                                self.try_execute(&mut vm, &scheduler, tx_version)
                            }
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
                        if self.abort_reason.get().is_some() {
                            break;
                        }

                        if task.is_none() {
                            task = scheduler.next_task();
                        }
                    }
                });
            }
        });

        if let Some(abort_reason) = self.abort_reason.take() {
            match abort_reason {
                AbortReason::FallbackToSequential => {
                    self.dropper.drop((mv_memory, scheduler));
                    return execute_revm_sequential(chain, storage, spec_id, block_env, txs);
                }
                AbortReason::ExecutionError(err) => {
                    self.dropper.drop((mv_memory, scheduler));
                    return Err(PevmError::ExecutionError(err));
                }
            }
        }

        let mut fully_evaluated_results = Vec::with_capacity(block_size);
        let mut cumulative_gas_used: u64 = 0;
        // After thread::scope returns we hold exclusive access to self.execution_results,
        // so we can take() through &mut without any locking.
        for slot in self.execution_results.iter_mut().take(block_size) {
            let mut execution_result = slot.take().unwrap();
            cumulative_gas_used =
                cumulative_gas_used.saturating_add(execution_result.receipt.cumulative_gas_used);
            execution_result.receipt.cumulative_gas_used = cumulative_gas_used;
            fully_evaluated_results.push(execution_result);
        }

        // We fully evaluate (the balance and nonce of) the beneficiary account
        // and raw transfer recipients that may have been atomically updated.
        for address in mv_memory.consume_lazy_addresses() {
            let location_hash = hash_deterministic(MemoryLocation::Basic(address));

            let Some(entries) = mv_memory.data.get(&location_hash) else {
                continue;
            };

            // Load code once — lazy addresses can be contracts (e.g. fee recipient).
            let code_hash = match storage.code_hash(&address) {
                Ok(ch) => ch,
                Err(err) => return Err(PevmError::StorageError(err.to_string())),
            };
            let code = if let Some(ch) = &code_hash {
                match storage.code_by_hash(ch) {
                    Ok(c) => c,
                    Err(err) => return Err(PevmError::StorageError(err.to_string())),
                }
            } else {
                None
            };
            let eip161 = chain.is_eip_161_enabled(spec_id);

            match entries.value() {
                MvEntries::Dense(_) => {
                    // Dense path: all lazy addresses land here after add_lazy_addresses
                    // upgrades them. Contiguous slice enables cache-friendly sequential
                    // prefix-sum and a parallel rayon state update.
                    let (mut balance, mut nonce) = storage
                        .basic(&address)
                        .ok()
                        .flatten()
                        .map(|acc| (acc.balance, acc.nonce))
                        .unwrap_or_default();

                    // Sequential pass: compute running (balance, nonce) after each tx.
                    // Errors (insufficient balance) are returned immediately.
                    let mut prefix_states: Vec<Option<(U256, u64)>> =
                        Vec::with_capacity(block_size);
                    for (tx_idx, entry_opt) in entries.dense_iter().enumerate() {
                        let Some(entry) = entry_opt else {
                            prefix_states.push(None);
                            continue;
                        };
                        match entry {
                            MemoryEntry::Data(_, MemoryValue::Basic(info)) => {
                                // We fall back to sequential on self-destruct, so empty is a bug.
                                debug_assert!(!(info.balance.is_zero() && info.nonce == 0));
                                balance = info.balance;
                                nonce = info.nonce;
                            }
                            MemoryEntry::Data(_, MemoryValue::LazyRecipient(addition)) => {
                                balance = balance.saturating_add(*addition);
                            }
                            MemoryEntry::Data(_, MemoryValue::LazySender(subtraction)) => {
                                // Re-do sender balance checks: we mocked MAX balance during
                                // execution so revm skipped these. Can't share revm's helpers.
                                let tx = chain.tx_env(unsafe { txs.get_unchecked(tx_idx) });
                                let mut max_fee = U256::from(tx.gas_limit)
                                    .saturating_mul(U256::from(tx.gas_price))
                                    .saturating_add(tx.value);
                                max_fee = max_fee.saturating_add(
                                    U256::from(tx.total_blob_gas())
                                        .saturating_mul(U256::from(tx.max_fee_per_blob_gas)),
                                );
                                if balance < max_fee {
                                    return Err(ExecutionError::Transaction(
                                        InvalidTransaction::LackOfFundForMaxFee {
                                            balance: Box::new(balance),
                                            fee: Box::new(max_fee),
                                        },
                                    ))?;
                                }
                                balance = balance.saturating_sub(*subtraction);
                                nonce += 1;
                                if tx.caller == address {
                                    let executed_nonce = if nonce == 0 {
                                        return Err(PevmError::UnreachableError);
                                    } else {
                                        nonce - 1
                                    };
                                    if tx.nonce != executed_nonce {
                                        return Err(PevmError::NonceMismatch {
                                            tx_idx,
                                            tx_nonce: tx.nonce,
                                            executed_nonce,
                                        });
                                    }
                                }
                            }
                            _ => unreachable!(),
                        }
                        prefix_states.push(Some((balance, nonce)));
                    }

                    // Parallel pass: apply (balance, nonce) to each tx_result's state.
                    fully_evaluated_results
                        .par_iter_mut()
                        .zip(prefix_states.par_iter())
                        .for_each(|(tx_result, state_opt)| {
                            let Some(&(bal, nonce)) = state_opt.as_ref() else {
                                return;
                            };
                            let account = tx_result.state.entry(address).or_default();
                            if eip161 && code_hash.is_none() && nonce == 0 && bal == U256::ZERO {
                                *account = None;
                            } else if let Some(existing) = account {
                                existing.balance = bal;
                                existing.nonce = nonce;
                            } else {
                                *account = Some(EvmAccount {
                                    balance: bal,
                                    nonce,
                                    code_hash,
                                    code: code.clone(),
                                    storage: HashMap::default(),
                                });
                            }
                        });
                }
                MvEntries::Sparse(_) => {
                    unreachable!("Lazy addresses should have been upgraded to dense entries in add_lazy_addresses, so we shouldn't have any sparse entries left. Found sparse entries for address {:?} with code hash {:?} and code {:?}.", address, code_hash, code);
                }
            }
        }

        self.dropper.drop((mv_memory, scheduler));

        Ok(fully_evaluated_results)
    }

    fn try_execute<'a, S: Storage, C: PevmChain>(
        &self,
        vm: &mut Vm<'a, S, C>,
        scheduler: &Scheduler,
        tx_version: TxVersion,
    ) -> Option<Task> {
        loop {
            return match vm.execute(&tx_version) {
                Err(VmExecutionError::Retry) => {
                    if self.abort_reason.get().is_none() {
                        continue;
                    }
                    None
                }
                Err(VmExecutionError::FallbackToSequential) => {
                    scheduler.abort();
                    self.abort_reason
                        .get_or_init(|| AbortReason::FallbackToSequential);
                    None
                }
                Err(VmExecutionError::Blocking(blocking_tx_idx)) => {
                    if !scheduler.add_dependency(tx_version.tx_idx, blocking_tx_idx)
                        && self.abort_reason.get().is_none()
                    {
                        // Retry the execution immediately if the blocking transaction was
                        // re-executed by the time we can add it as a dependency.
                        continue;
                    }
                    None
                }
                Err(VmExecutionError::ExecutionError(err)) => {
                    scheduler.abort();
                    self.abort_reason
                        .get_or_init(|| AbortReason::ExecutionError(err));
                    None
                }
                Ok(VmExecutionResult {
                    execution_result,
                    flags,
                }) => {
                    // SAFETY: scheduler ensures only one thread executes a given tx_idx
                    // at a time, so this lockless write never races with another writer.
                    unsafe { self.execution_results.get_unchecked(tx_version.tx_idx) }
                        .write(execution_result);
                    scheduler.finish_execution(tx_version, flags)
                }
            };
        }
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

/// Execute REVM transactions sequentially.
// Useful for falling back for (small) blocks with many dependencies.
// TODO: Use this for a long chain of sequential transactions even in parallel mode.
pub fn execute_revm_sequential<S: Storage + Debug, C: PevmChain>(
    chain: &C,
    storage: &S,
    spec_id: C::EvmSpecId,
    block_env: BlockEnv,
    txs: Vec<C::EvmTx>,
) -> PevmResult<C> {
    let db = CacheDB::new(StorageWrapper(storage));
    let mut evm = chain.build_evm(spec_id, block_env, db);

    let mut results: Vec<PevmTxExecutionResult> = Vec::with_capacity(txs.len());
    let mut cumulative_gas_used: u64 = 0;
    for tx in txs {
        // TODO: More concrete error type
        let result_and_state = evm
            .transact(tx)
            .map_err(|err| ExecutionError::Custom(err.to_string()))?;

        evm.ctx().db_mut().commit(result_and_state.state.clone());

        let mut execution_result =
            PevmTxExecutionResult::from_revm(chain, spec_id, result_and_state);

        cumulative_gas_used =
            cumulative_gas_used.saturating_add(execution_result.receipt.cumulative_gas_used);
        execution_result.receipt.cumulative_gas_used = cumulative_gas_used;

        results.push(execution_result);
    }
    Ok(results)
}
