use std::{cell::OnceCell, sync::Arc};

use ahash::AHashMap;
use revm::{
    primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, EVMError, ResultAndState,
        SpecId::{self, LONDON},
        TxEnv, B256, U256,
    },
    Database, Evm, Handler,
};

use crate::{
    mv_memory::{MvMemory, ReadMemoryResult},
    MemoryEntry, MemoryLocation, MemoryValue, ReadError, ReadOrigin, ReadSet, Storage, TxIdx,
    WriteSet,
};

/// The execution error from the underlying EVM executor.
// Will there be DB errors outside of read?
pub type ExecutionError = EVMError<ReadError>;

pub(crate) enum VmExecutionResult {
    ReadError {
        blocking_tx_idx: TxIdx,
    },
    ExecutionError(ExecutionError),
    Ok {
        result_and_state: ResultAndState,
        read_set: ReadSet,
        write_set: WriteSet,
    },
}

// A database interface that intercepts reads while executing a specific
// transaction with revm. It provides values from the multi-version data
// structure & storage, and tracks the read set of the current execution.
struct VmDb<S: Storage> {
    beneficiary_address: Address,
    beneficiary_location: MemoryLocation,
    tx_idx: TxIdx,
    mv_memory: Arc<MvMemory>,
    storage: Arc<S>,
    read_set: ReadSet,
}

impl<S: Storage> VmDb<S> {
    fn new(
        beneficiary_address: Address,
        tx_idx: TxIdx,
        mv_memory: Arc<MvMemory>,
        storage: Arc<S>,
    ) -> Self {
        Self {
            beneficiary_address,
            beneficiary_location: MemoryLocation::Basic(beneficiary_address),
            tx_idx,
            mv_memory,
            storage,
            read_set: ReadSet {
                // There are at least two locations most of the time: the sender
                // and the recipient accounts.
                common: Vec::with_capacity(2),
                beneficiary: Vec::new(),
            },
        }
    }

    fn read(
        &mut self,
        location: MemoryLocation,
        update_read_set: bool,
    ) -> Result<MemoryValue, ReadError> {
        if location == self.beneficiary_location {
            return self.read_beneficiary();
        }

        match self.mv_memory.read_closest(&location, self.tx_idx) {
            ReadMemoryResult::ReadError { blocking_tx_idx } => {
                Err(ReadError::BlockingIndex(blocking_tx_idx))
            }
            ReadMemoryResult::NotFound => {
                if update_read_set {
                    self.read_set
                        .common
                        .push((location.clone(), ReadOrigin::Storage));
                }
                match location {
                    MemoryLocation::Basic(address) => match self.storage.basic(address) {
                        Ok(Some(account)) => Ok(MemoryValue::Basic(Box::new(account.into()))),
                        Ok(None) => Err(ReadError::NotFound),
                        Err(err) => Err(ReadError::StorageError(format!("{err:?}"))),
                    },
                    MemoryLocation::Storage((address, index)) => self
                        .storage
                        .storage(address, index)
                        .map(MemoryValue::Storage)
                        .map_err(|err| ReadError::StorageError(format!("{err:?}"))),
                }
            }
            ReadMemoryResult::Ok { version, value } => {
                if update_read_set {
                    self.read_set
                        .common
                        .push((location, ReadOrigin::MvMemory(version)));
                }
                Ok(value)
            }
        }
    }

    fn read_beneficiary(&mut self) -> Result<MemoryValue, ReadError> {
        if self.tx_idx == 0 {
            return Ok(MemoryValue::Basic(Box::new(
                self.read_beneficiary_from_storage()?,
            )));
        }

        // We simply register this transaction as dependency of the previous in
        // the racing cases that the previous transactions aren't yet ready.
        // TODO: Only reschedule up to a certain number of times.
        let reschedule = Err(ReadError::BlockingIndex(self.tx_idx - 1));

        let Some(written_beneficiary) = self.mv_memory.read_beneficiary() else {
            return reschedule;
        };

        let mut balance_addition = U256::ZERO;
        let mut tx_idx = self.tx_idx - 1;
        loop {
            match written_beneficiary.get(&tx_idx) {
                Some(MemoryEntry::Data(tx_incarnation, value)) => {
                    self.read_set
                        .beneficiary
                        .push(ReadOrigin::MvMemory(crate::TxVersion {
                            tx_idx,
                            tx_incarnation: *tx_incarnation,
                        }));
                    match value {
                        MemoryValue::Basic(account) => {
                            let mut account = account.clone();
                            account.balance += balance_addition;
                            return Ok(MemoryValue::Basic(account));
                        }
                        MemoryValue::LazyBeneficiaryBalance(addition) => {
                            // TODO: Be careful with overflows
                            balance_addition += addition;
                        }
                        _ => unreachable!("Unexpected storage value for beneficiary account info"),
                    }
                }
                _ => return reschedule,
            }
            if tx_idx == 0 {
                let mut account = self.read_beneficiary_from_storage()?;
                account.balance += balance_addition;
                return Ok(MemoryValue::Basic(Box::new(account)));
            }
            tx_idx -= 1;
        }
    }

    fn read_beneficiary_from_storage(&self) -> Result<AccountInfo, ReadError> {
        match self.storage.basic(self.beneficiary_address) {
            Ok(Some(account)) => Ok(account.into()),
            Ok(None) => Err(ReadError::NotFound),
            Err(err) => Err(ReadError::StorageError(format!("{err:?}"))),
        }
    }
}

impl<S: Storage> Database for VmDb<S> {
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
        if address == self.beneficiary_address && is_preload {
            return Ok(Some(AccountInfo::default()));
        }
        match self.read(MemoryLocation::Basic(address), !is_preload) {
            Ok(MemoryValue::Basic(value)) => Ok(Some(*value)),
            Err(ReadError::NotFound) => Ok(None),
            Err(err) => Err(err),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.storage
            .code_by_hash(code_hash)
            .map(Bytecode::new_raw)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn has_storage(&mut self, address: Address) -> Result<bool, Self::Error> {
        self.storage
            .has_storage(address)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.read(MemoryLocation::Storage((address, index)), true) {
            Err(err) => Err(err),
            Ok(MemoryValue::Storage(value)) => Ok(value),
            _ => Err(ReadError::InvalidMemoryLocationType),
        }
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.storage
            .block_hash(number)
            .map_err(|err| ReadError::StorageError(format!("{err:?}")))
    }
}

// The VM describes how to read values to execute transactions. Also, it
// captures the read & write sets of each execution. Note that a single
// `Vm` can be shared among threads.
pub(crate) struct Vm<S: Storage> {
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
    storage: Arc<S>,
    mv_memory: Arc<MvMemory>,
}

impl<S: Storage> Vm<S> {
    pub(crate) fn new(
        spec_id: SpecId,
        block_env: BlockEnv,
        txs: Vec<TxEnv>,
        storage: S,
        mv_memory: Arc<MvMemory>,
    ) -> Self {
        Self {
            spec_id,
            block_env,
            txs,
            storage: Arc::new(storage),
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
            self.block_env.coinbase,
            tx_idx,
            self.mv_memory.clone(),
            self.storage.clone(),
        );
        // The amount this transaction needs to pay to the beneficiary account for
        // atomic update.
        let mut gas_payment = OnceCell::new();
        // TODO: Support OP handlers
        let mut handler = Handler::mainnet_with_spec(self.spec_id);
        handler.post_execution.reward_beneficiary = Arc::new(|context, gas| {
            let mut gas_price = context.evm.env.effective_gas_price();
            if self.spec_id.is_enabled_in(LONDON) {
                gas_price = gas_price.saturating_sub(context.evm.env.block.basefee);
            }
            gas_payment
                .set(gas_price * U256::from(gas.spent() - gas.refunded() as u64))
                .unwrap();
            Ok(())
        });

        let evm_result = Evm::builder()
            .with_db(&mut db)
            .with_block_env(self.block_env.clone())
            // SATEFY: A correct scheduler would guarantee this index to be inbound.
            .with_tx_env(unsafe { self.txs.get_unchecked(tx_idx) }.clone())
            .with_handler(handler)
            .build()
            .transact();

        match evm_result {
            Ok(result_and_state) => {
                // A hash map is critical as there can be multiple state transitions
                // of the same location in a transaction (think internal txs)!
                // We only care about the latest state.
                let mut write_set: AHashMap<MemoryLocation, MemoryValue> =
                    // There are at least three locations most of the time: the sender,
                    // the recipient, and the beneficiary accounts.
                    AHashMap::with_capacity(3);
                for (address, account) in result_and_state.state.iter() {
                    if account.is_info_changed() {
                        // TODO: More granularity here to ensure we only notify new
                        // memory writes, for instance, only an account's balance instead
                        // of the whole account. That way we may also generalize beneficiary
                        // balance's lazy update behaviour into `MemoryValue` for more use cases.
                        // TODO: Confirm that we're not missing anything, like bytecode.
                        let mut account_info = account.info.clone();
                        if address == &self.block_env.coinbase {
                            account_info.balance += gas_payment.take().unwrap();
                        }
                        write_set.insert(
                            MemoryLocation::Basic(*address),
                            MemoryValue::Basic(Box::new(account_info)),
                        );
                    }
                    for (slot, value) in account.changed_storage_slots() {
                        write_set.insert(
                            MemoryLocation::Storage((*address, *slot)),
                            MemoryValue::Storage(value.present_value),
                        );
                    }
                }

                // A non-existent explicit write hasn't taken the cell.
                if let Some(gas_payment) = gas_payment.into_inner() {
                    write_set.insert(
                        MemoryLocation::Basic(self.block_env.coinbase),
                        MemoryValue::LazyBeneficiaryBalance(gas_payment),
                    );
                }

                VmExecutionResult::Ok {
                    result_and_state,
                    read_set: db.read_set,
                    write_set,
                }
            }
            Err(EVMError::Database(ReadError::BlockingIndex(blocking_tx_idx))) => {
                VmExecutionResult::ReadError { blocking_tx_idx }
            }
            Err(err) => VmExecutionResult::ExecutionError(err),
        }
    }
}
