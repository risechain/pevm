use core::time;
use std::{cell::RefCell, sync::Arc, thread};

use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CancunSpec, EVMError, ResultAndState, TxEnv,
        B256, U256,
    },
    Database, Evm, Handler,
};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    MemoryLocation, MemoryValue, ReadError, ReadOrigin, ReadSet, Storage, TxIdx, WriteSet,
};

pub(crate) enum VmExecutionResult {
    ReadError {
        blocking_tx_idx: TxIdx,
    },
    Ok {
        result_and_state: ResultAndState,
        read_set: ReadSet,
        write_set: WriteSet,
    },
}

// A database interface that intercepts reads while executing a specific
// transaction with revm. It provides values from the multi-version data
// structure & storage, and tracks the read set of the current execution.
struct VmDb {
    block_env: BlockEnv,
    tx_idx: TxIdx,
    mv_memory: Arc<MvMemory>,
    storage: Arc<Storage>,
    read_set: ReadSet,
}

impl VmDb {
    fn new(
        block_env: BlockEnv,
        tx_idx: TxIdx,
        mv_memory: Arc<MvMemory>,
        storage: Arc<Storage>,
    ) -> Self {
        Self {
            block_env,
            tx_idx,
            mv_memory,
            storage,
            read_set: Vec::new(),
        }
    }

    // TODO: Distinguish beneficiary balance reads to call their dedicated
    // read function. Should we branch here or just init MvMemory with
    // the benediciary address?
    fn read(
        &mut self,
        location: &MemoryLocation,
        tx_idx: TxIdx,
        update_read_set: bool,
    ) -> Result<MemoryValue, ReadError> {
        // Dedicated handling for the beneficiary account
        // TODO: Refactor & find a better place for this specific logic
        if let MemoryLocation::Basic(address) = *location {
            if address == self.block_env.coinbase {
                if tx_idx == 0 {
                    if update_read_set {
                        self.read_set.push((location.clone(), ReadOrigin::Storage));
                    }
                    return self.storage.basic(address).map(MemoryValue::Basic);
                }
                match self.mv_memory.read_absolute(location, tx_idx - 1) {
                    ReadMemoryResult::Ok { version, value } => {
                        if update_read_set {
                            self.read_set
                                .push((location.clone(), ReadOrigin::MvMemory(version)));
                        }
                        match value {
                            MemoryValue::Basic(account) => return Ok(MemoryValue::Basic(account)),
                            MemoryValue::LazyBeneficiaryBalance(addition) => {
                                // TODO: Better error handling
                                match self.read(location, tx_idx - 1, false) {
                                    Ok(MemoryValue::Basic(mut beneficiary_account)) => {
                                        // TODO: Write this new absolute value to MvMemory
                                        // to avoid future recalculations.
                                        beneficiary_account.balance += addition;
                                        return Ok(MemoryValue::Basic(beneficiary_account));
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        // Wait for the previous transaction to complete processing
                        // the beneficiary account.
                        // TODO: Ideally, we can detect and abort if the wait is
                        // expected to be long, like via recursion depth.
                        // Sad: Bump delay time when facing stack overflow..
                        thread::sleep(time::Duration::from_nanos(8));
                        return self.read(location, tx_idx, update_read_set);
                    }
                }
            }
        }

        // Main handling for BlockSTM
        match self.mv_memory.read_closest(location, tx_idx) {
            ReadMemoryResult::ReadError { blocking_tx_idx } => {
                Err(ReadError::BlockingIndex(blocking_tx_idx))
            }
            ReadMemoryResult::NotFound => {
                if update_read_set {
                    self.read_set.push((location.clone(), ReadOrigin::Storage));
                }
                match location {
                    MemoryLocation::Basic(address) => {
                        self.storage.basic(*address).map(MemoryValue::Basic)
                    }
                    MemoryLocation::Storage((address, index)) => self
                        .storage
                        .storage(*address, *index)
                        .map(MemoryValue::Storage),
                }
            }
            ReadMemoryResult::Ok { version, value } => {
                if update_read_set {
                    self.read_set
                        .push((location.clone(), ReadOrigin::MvMemory(version)));
                }
                Ok(value)
            }
        }
    }
}

impl Database for VmDb {
    type Error = ReadError;

    // TODO: More granularity here to ensure we only record dependencies for,
    // for instance, only an account's balance instead of the whole account
    // info. That way we may also generalize beneficiary balance's lazy update
    // behaviour into `MemoryValue` for more use cases.
    fn basic(
        &mut self,
        address: Address,
        // TODO: Better way for REVM to notifiy explicit reads
        is_preload: bool,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        // We preload a mock beneficiary account, to only lazy evaluate it on
        // explicit reads and once BlockSTM is completed.
        if address == self.block_env.coinbase && is_preload {
            return Ok(Some(AccountInfo::default()));
        }
        match self.read(&MemoryLocation::Basic(address), self.tx_idx, !is_preload) {
            Err(err) => Err(err),
            Ok(MemoryValue::Basic(value)) => Ok(Some(value)),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.storage.code_by_hash(code_hash)
    }

    fn storage(
        &mut self,
        address: Address,
        index: U256,
        // TODO: Better way for REVM to notifiy explicit reads
        is_preload: bool,
    ) -> Result<U256, Self::Error> {
        match self.read(
            &MemoryLocation::Storage((address, index)),
            self.tx_idx,
            !is_preload,
        ) {
            Err(err) => Err(err),
            Ok(MemoryValue::Storage(value)) => Ok(value),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.storage.block_hash(number)
    }
}

// The VM describes how to read values to execute transactions. Also, it
// captures the read & write sets of each execution.
pub(crate) struct Vm {
    storage: Arc<Storage>,
    block_env: BlockEnv,
    txs: Arc<Vec<TxEnv>>,
    mv_memory: Arc<MvMemory>,
}

impl Vm {
    pub(crate) fn new(
        storage: Arc<Storage>,
        block_env: BlockEnv,
        txs: Arc<Vec<TxEnv>>,
        mv_memory: Arc<MvMemory>,
    ) -> Self {
        Self {
            storage,
            block_env,
            txs,
            mv_memory,
        }
    }

    // Execute a transaction. This can read from memory but cannot modify any state.
    // A successful execution returns:
    //   - A write-set consisting of memory locations and their updated values.
    //   - A read-set consisting of memory locations read during incarnation and its
    //   origin.
    //
    // An execution may observe a read dependency on a lower transaction. This happens
    // when the last incarnation of the dependency wrote to a memory location that
    // this transaction reads, but it aborted before the read. In this case, the
    // depedency index is returend via `blocking_tx_idx`. An execution task for this
    // this transaction is re-scheduled after the blocking dependency finishes its
    // next incarnation.
    //
    // When a transaction attempts to write a value to a location, the location and
    // value are added to the write set, possibly replacing a pair with a prior value
    // (if it is not the first time the transaction wrote to this location during the
    // execution).
    #[allow(clippy::arc_with_non_send_sync)] // TODO: Fix at REVM?
    pub(crate) fn execute(&self, tx_idx: TxIdx) -> VmExecutionResult {
        let mut db = VmDb::new(
            self.block_env.clone(),
            tx_idx,
            self.mv_memory.clone(),
            self.storage.clone(),
        );
        // The amount this transaction needs to pay to the beneficiary account for
        // atomic update.
        let gas_payment = RefCell::new(U256::ZERO);
        // TODO: Support OP & receive Spec as a BlockSTM input.
        let mut handler = Handler::mainnet::<CancunSpec>();
        // TODO: Bring to `self` instead of constructing every call?
        handler.post_execution.reward_beneficiary = Arc::new(|context, gas| {
            *gas_payment.borrow_mut() = context
                .evm
                .env
                .effective_gas_price()
                .saturating_sub(context.evm.env.block.basefee)
                * U256::from(gas.spent() - gas.refunded() as u64);
            Ok(())
        });

        let mut evm = Evm::builder()
            .with_db(&mut db)
            .with_handler(handler)
            .with_block_env(self.block_env.clone())
            .with_tx_env(self.txs.get(tx_idx).unwrap().clone())
            .build();

        let evm_result = evm.transact();
        drop(evm); // to reclaim the DB

        match evm_result {
            Ok(result_and_state) => {
                let mut explicitly_wrote_to_coinbase = false;
                let mut write_set: Vec<(MemoryLocation, MemoryValue)> = result_and_state
                    .state
                    .iter()
                    .flat_map(|(address, account)| {
                        let mut writes = Vec::new();
                        // TODO: Verify that depending on this touched status is correct!
                        if account.is_touched() {
                            // TODO: More granularity here to ensure we only notify new
                            // memory writes, for instance, only an account's balance instead
                            // of the whole account. That way we may also generalize beneficiary
                            // balance's lazy update behaviour into `MemoryValue` for more use cases.
                            // TODO: Confirm that we're not missing anything, like bytecode.
                            let mut account_info = account.info.clone();
                            if address == &self.block_env.coinbase {
                                account_info.balance += *gas_payment.borrow();
                                explicitly_wrote_to_coinbase = true;
                            }
                            writes.push((
                                MemoryLocation::Basic(*address),
                                MemoryValue::Basic(account_info),
                            ));
                            for (slot, value) in account.changed_storage_slots() {
                                writes.push((
                                    MemoryLocation::Storage((*address, *slot)),
                                    MemoryValue::Storage(value.present_value),
                                ));
                            }
                        }
                        writes
                    })
                    .collect();

                if !explicitly_wrote_to_coinbase {
                    write_set.push((
                        MemoryLocation::Basic(self.block_env.coinbase),
                        MemoryValue::LazyBeneficiaryBalance(*gas_payment.borrow()),
                    ));
                }

                VmExecutionResult::Ok {
                    result_and_state: result_and_state.clone(),
                    read_set: db.read_set,
                    write_set,
                }
            }
            Err(EVMError::Database(ReadError::BlockingIndex(blocking_tx_idx))) => {
                VmExecutionResult::ReadError { blocking_tx_idx }
            }
            // TODO: More error handling here
            _ => todo!(),
        }
    }
}
