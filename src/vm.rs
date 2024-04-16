use std::sync::Arc;

use revm::{
    primitives::{AccountInfo, Address, Bytecode, EVMError, ResultAndState, TxEnv, B256, U256},
    Database, DatabaseRef, Evm,
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
//  transaction with revm.
struct VmDb {
    tx_idx: TxIdx,
    mv_memory: Arc<MvMemory>,
    storage: Arc<Storage>,
    read_set: ReadSet,
}

impl VmDb {
    fn new(tx_idx: TxIdx, mv_memory: Arc<MvMemory>, storage: Arc<Storage>) -> Self {
        Self {
            tx_idx,
            mv_memory,
            storage,
            read_set: Vec::new(),
        }
    }

    fn read(&mut self, location: MemoryLocation) -> Result<MemoryValue, ReadError> {
        // TODO: Better error handling
        match self.mv_memory.read(&location, self.tx_idx) {
            ReadMemoryResult::ReadError { blocking_tx_idx } => {
                Err(ReadError::BlockingIndex(blocking_tx_idx))
            }
            ReadMemoryResult::NotFound => {
                self.read_set.push((location.clone(), ReadOrigin::Storage));
                match location {
                    MemoryLocation::Basic(address) => {
                        self.storage.basic_ref(address).map(MemoryValue::Basic)
                    }
                    MemoryLocation::Storage((address, index)) => self
                        .storage
                        .storage_ref(address, index)
                        .map(MemoryValue::Storage),
                }
            }
            ReadMemoryResult::Ok { version, value } => {
                self.read_set
                    .push((location, ReadOrigin::MvMemory(version)));
                Ok(value)
            }
        }
    }
}

impl Database for VmDb {
    type Error = ReadError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match self.read(MemoryLocation::Basic(address)) {
            Ok(MemoryValue::Basic(value)) => Ok(value),
            Ok(MemoryValue::Storage(_)) => Err(ReadError::InvalidMemoryLocationType),
            Err(err) => Err(err),
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.storage.code_by_hash_ref(code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.read(MemoryLocation::Storage((address, index))) {
            Ok(MemoryValue::Basic(_)) => Err(ReadError::InvalidMemoryLocationType),
            Ok(MemoryValue::Storage(value)) => Ok(value),
            Err(err) => Err(err),
        }
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.storage.block_hash_ref(number)
    }
}

// The VM describes how reads & writes are instrumented during transaction
// execution
pub(crate) struct Vm {
    // NOTE: We don't need the ability to borrow mutably here.
    storage: Arc<Storage>,
    txs: Arc<Vec<TxEnv>>,
    mv_memory: Arc<MvMemory>,
}

impl Vm {
    pub fn new(storage: Arc<Storage>, txs: Arc<Vec<TxEnv>>, mv_memory: Arc<MvMemory>) -> Self {
        Self {
            storage,
            txs,
            mv_memory,
        }
    }

    // This can read from memory but cannot modify any state.
    // A successful execution returns:
    //   - A write-set, consisting of memory locations and their updated values.
    //   - A read-set, consisting of memory locations read during incarnation,
    //   each associated with whether a value was read from MvMemory or Storage.
    //   In the former case, the version of the transaction execution that previously
    //   wrote the value.
    // An execution may observe a read dependency on a lower transaction. This happens
    // when the last incarnation of the dependency wrote to a memory location that
    // this transaction reads, but it aborted before the read. In this case, the
    // depedency index is returend via `blocking_tx_idx`. An execution task for this
    // this transaction is re-scheduled after the blocking dependency finishes its
    // next incarnation.
    //
    // When a transaction attempts to write a value to a location, the (location,
    // value) pair is added to the write set, possibly replacing a pair with a prior
    // value (if it is not the first time the transaction wrote to this location
    // during the execution).
    // When a transaction attempts to read a location, if the location is already
    // in the write-set then the VM reads the corresponding value (that the
    // transaction itself wrote). Otherwise, it tries to read from MvMemory and
    // Storage.
    pub(crate) fn execute(&self, tx_idx: TxIdx) -> VmExecutionResult {
        let mut db = VmDb::new(tx_idx, self.mv_memory.clone(), self.storage.clone());

        let mut evm = Evm::builder()
            .with_db(&mut db)
            .with_tx_env(self.txs.get(tx_idx).unwrap().clone())
            .build();

        let evm_result = evm.transact();
        drop(evm); // to reclaim the DB

        match evm_result {
            // TODO: Do we need to look into result here?
            Ok(result_and_state) => VmExecutionResult::Ok {
                result_and_state: result_and_state.clone(),
                // TODO: Confirm that this is correct. For instance,
                // that REVM doesn't use some internal values and hence
                // miss some `VmDb` calls that populate the read set.
                read_set: db.read_set,
                write_set: result_and_state
                    .state
                    .iter()
                    .flat_map(|(address, account)| {
                        // TODO: More granularity here to ensure we only notify
                        // new value writes and properly skip old-value locations.
                        // TODO: Confirm that we're not missing anything, like bytecode.
                        let mut writes = vec![];
                        // TODO: Why are there empty accounts here in the first
                        // place?
                        if !account.is_empty() {
                            // TODO: Checking if the account's basic info is changed
                            // before registering it as a new write.
                            writes.push((
                                MemoryLocation::Basic(address.clone()),
                                MemoryValue::Basic(Some(account.info.clone())),
                            ));
                            for (slot, value) in account.changed_storage_slots() {
                                writes.push((
                                    MemoryLocation::Storage((address.clone(), slot.clone())),
                                    MemoryValue::Storage(value.present_value),
                                ));
                            }
                        }
                        writes
                    })
                    .collect(),
            },
            Err(EVMError::Database(ReadError::BlockingIndex(blocking_tx_idx))) => {
                VmExecutionResult::ReadError { blocking_tx_idx }
            }
            // TODO: More error handling here
            _ => todo!(),
        }
    }
}
