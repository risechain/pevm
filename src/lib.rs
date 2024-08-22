//! Blazingly fast Parallel EVM in Rust.

// TODO: Better types & API for third-party integration

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy_primitives::{Address, B256, U256};
use smallvec::SmallVec;

// We take the last 8 bytes of an address as its hash. This
// seems fine as the addresses themselves are hash suffixes,
// and precomiles' suffix should be unique, too.
// TODO: Make sure this is acceptable for production
#[derive(Default)]
struct AddressHasher(u64);
impl Hasher for AddressHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut suffix = [0u8; 8];
        suffix.copy_from_slice(&bytes[bytes.len() - 8..]);
        self.0 = u64::from_be_bytes(suffix);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}
type BuildAddressHasher = BuildHasherDefault<AddressHasher>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MemoryLocation {
    // TODO: Separate an account's balance and nonce?
    Basic(Address),
    CodeHash(Address),
    Storage(Address, U256),
}

// We only need the full memory location to read from storage.
// We then identify the locations with its hash in the multi-version
// data, write and read sets, which is much faster than rehashing
// on every single lookup & validation.
type MemoryLocationHash = u64;

// This is primarily used for memory location hash, but can also be used for
// transaction indexes, etc.
#[derive(Default)]
struct IdentityHasher(u64);
impl Hasher for IdentityHasher {
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }
    fn write_usize(&mut self, id: usize) {
        self.0 = id as u64;
    }
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!()
    }
}
type BuildIdentityHasher = BuildHasherDefault<IdentityHasher>;

// TODO: It would be nice if we could tie the different cases of
// memory locations & values at the type level, to prevent lots of
// matches & potentially dangerous mismatch mistakes.
#[derive(Debug, Clone)]
enum MemoryValue {
    Basic(AccountBasic),
    CodeHash(Option<B256>),
    Storage(U256),
    // We lazily update the beneficiary balance to avoid continuous
    // dependencies as all transactions read and write to it. We also
    // lazy update the senders & recipients of raw transfers, which are
    // also common (popular CEX addresses, airdrops, etc).
    // We fully evaluate these account states at the end of the block or
    // when there is an explicit read.
    // Explicit balance addition.
    LazyRecipient(U256),
    // Explicit balance subtraction & implicit nonce increment.
    LazySender(U256),
}

#[derive(Debug)]
enum MemoryEntry {
    Data(TxIncarnation, MemoryValue),
    // When an incarnation is aborted due to a validation failure, the
    // entries in the multi-version data structure corresponding to its
    // write set are replaced with this special ESTIMATE marker.
    // This signifies that the next incarnation is estimated to write to
    // the same memory locations. An incarnation stops and is immediately
    // aborted whenever it reads a value marked as an ESTIMATE written by
    // a lower transaction, instead of potentially wasting a full execution
    // and aborting during validation.
    // The ESTIMATE markers that are not overwritten are removed by the next
    // incarnation.
    Estimate,
}

// The index of the transaction in the block.
// TODO: Consider downsizing to [u32].
type TxIdx = usize;

// The i-th time a transaction is re-executed, counting from 0.
// TODO: Consider downsizing to [u32].
type TxIncarnation = usize;

// - ReadyToExecute(i) --try_incarnate--> Executing(i)
// Non-blocked execution:
//   - Executing(i) --finish_execution--> Executed(i)
//   - Executed(i) --finish_validation--> Validated(i)
//   - Executed/Validated(i) --try_validation_abort--> Aborting(i)
//   - Aborted(i) --finish_validation(w.aborted=true)--> ReadyToExecute(i+1)
// Blocked execution:
//   - Executing(i) --add_dependency--> Aborting(i)
//   - Aborting(i) --resume--> ReadyToExecute(i+1)
#[derive(PartialEq, Debug)]
#[repr(usize)]
enum IncarnationStatus {
    ReadyToExecute = 2,
    Executing = 3,
    Executed = 4,
    Validated = 5,
    Aborting = 6,
}

impl From<usize> for IncarnationStatus {
    fn from(value: usize) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl From<IncarnationStatus> for usize {
    fn from(value: IncarnationStatus) -> Self {
        value as usize
    }
}

struct TxStatusGuard<'a> {
    lock: &'a AtomicUsize,
    data: usize, // always equal to self.lock.load()
}

impl<'a> TxStatusGuard<'a> {
    fn new(lock: &'a AtomicUsize, data: usize) -> TxStatusGuard<'a> {
        Self { lock, data }
    }

    // fn get(&self) -> usize {
    //     self.data
    // }

    fn set(&mut self, data: usize) {
        self.data = data;
        self.lock.store(data, Ordering::Release);
    }

    fn status(&self) -> IncarnationStatus {
        IncarnationStatus::from(self.data & 0b111)
    }

    fn set_status(&mut self, status: IncarnationStatus) {
        self.set((self.data & !0b111) | usize::from(status))
    }

    fn incarnation(&self) -> TxIncarnation {
        self.data >> 4
    }

    // fn set_incarnation(&mut self, incarnation: TxIncarnation) {
    //     self.set((self.data & 0b1111) | (incarnation << 4))
    // }

    fn add_incarnation(&mut self, incarnation_to_add: TxIncarnation, status: IncarnationStatus) {
        let new_incarnation = (self.data >> 4) + incarnation_to_add;
        self.set((new_incarnation << 4) | (self.data & 0b1000) | usize::from(status));
    }
}

impl<'a> Drop for TxStatusGuard<'a> {
    fn drop(&mut self) {
        self.lock.fetch_and(!0b1000, Ordering::Release);
    }
}

#[derive(Debug)]
struct AtomicTxStatus(AtomicUsize);

impl AtomicTxStatus {
    fn new(incarnation: TxIncarnation, status: IncarnationStatus) -> Self {
        Self(AtomicUsize::new((incarnation << 4) | usize::from(status)))
    }

    fn lock(&self) -> Option<TxStatusGuard> {
        loop {
            let old_unlocked = self.0.load(Ordering::Relaxed) & !0b1000;
            let new_locked = old_unlocked | 0b1000;
            if self
                .0
                .compare_exchange_weak(
                    old_unlocked,
                    new_locked,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return Some(TxStatusGuard::new(&self.0, new_locked));
            } else {
                std::hint::spin_loop();
            }
        }
    }
}

// We maintain an in-memory multi-version data structure that stores for
// each memory location the latest value written per transaction, along
// with the associated transaction incarnation. When a transaction reads
// a memory location, it obtains from the multi-version data structure the
// value written to this location by the highest transaction that appears
// before it in the block, along with the associated version. If no previous
// transactions have written to a location, the value would be read from the
// storage state before block execution.
#[derive(Clone, Debug, PartialEq)]
struct TxVersion {
    tx_idx: TxIdx,
    tx_incarnation: TxIncarnation,
}

// The origin of a memory read. It could be from the live multi-version
// data structure or from storage (chain state before block execution).
#[derive(Debug, PartialEq)]
enum ReadOrigin {
    MvMemory(TxVersion),
    Storage,
}

// For validation: a list of read origins (previous transaction versions)
// for each read memory location.
type ReadSet = HashMap<MemoryLocationHash, SmallVec<[ReadOrigin; 1]>, BuildIdentityHasher>;

// The updates made by this transaction incarnation, which is applied
// to the multi-version data structure at the end of execution.
type WriteSet = Vec<(MemoryLocationHash, MemoryValue)>;

type NewLazyAddresses = Vec<Address>;

/// Errors when reading a memory location.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadError {
    /// Cannot read memory location from storage.
    StorageError(String),
    /// Memory location not found.
    NotFound,
    /// This memory location has been written by a lower transaction.
    BlockingIndex(TxIdx),
    /// There has been an inconsistent read like reading the same
    /// location from storage in the first call but from [VmMemory] in
    /// the next.
    InconsistentRead,
    /// Found an invalid nonce, like the first transaction of a sender
    /// not having a (+1) nonce from storage.
    /// TODO: Add the address and tx index to the error.
    InvalidNonce,
    /// Read a self-destructed account that is very hard to handle, as
    /// there is no performant way to mark all storage slots as cleared.
    SelfDestructedAccount,
    /// The stored memory value type doesn't match its location type.
    /// TODO: Handle this at the type level?
    InvalidMemoryLocationType,
}

// A scheduled worker task
// TODO: Add more useful work when there are idle workers like near
// the end of block execution, while waiting for a huge blocking
// transaction to resolve, etc.
#[derive(Debug)]
enum Task {
    Execution(TxVersion),
    Validation(TxVersion),
}

// This optimization is desired as we constantly index into many
// vectors of the block-size size. It can yield up to 5% improvement.
macro_rules! index_mutex {
    ($vec:expr, $index:expr) => {
        // SAFETY: A correct scheduler would not leak indexes larger
        // than the block size, which is the size of all vectors we
        // index via this macro. Otherwise, DO NOT USE!
        // TODO: Better error handling for the mutex.
        unsafe { $vec.get_unchecked($index).lock().unwrap() }
    };
}

pub mod chain;
mod compat;
mod mv_memory;
mod pevm;
pub use pevm::{execute, execute_revm_parallel, execute_revm_sequential, PevmError, PevmResult};
mod scheduler;
mod storage;
pub use storage::{
    AccountBasic, Bytecodes, EvmAccount, EvmCode, InMemoryStorage, RpcStorage, Storage,
    StorageWrapper,
};
mod vm;
pub use vm::{ExecutionError, PevmTxExecutionResult};
