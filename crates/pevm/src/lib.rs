//! Blazingly fast Parallel EVM in Rust.

// TODO: Better types & API for third-party integration

use std::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};

use alloy_primitives::{Address, B256, U256};
use bitflags::bitflags;
use hashbrown::HashMap;
use rustc_hash::FxBuildHasher;
use smallvec::SmallVec;

/// We use the last 8 bytes of an existing hash like address
/// or code hash instead of rehashing it.
// TODO: Make sure this is acceptable for production
#[derive(Debug, Default)]
pub struct SuffixHasher(u64);
impl Hasher for SuffixHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut suffix = [0u8; 8];
        suffix.copy_from_slice(&bytes[bytes.len() - 8..]);
        self.0 = u64::from_be_bytes(suffix);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// Build a suffix hasher
pub type BuildSuffixHasher = BuildHasherDefault<SuffixHasher>;

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

/// This is primarily used for memory location hash, but can also be used for
/// transaction indexes, etc.
#[derive(Debug, Default)]
pub struct IdentityHasher(u64);
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

/// Build an identity hasher
pub type BuildIdentityHasher = BuildHasherDefault<IdentityHasher>;

// TODO: Ensure it's not easy to hand-craft transactions and storage slots
// that can cause a lot of collisions that destroys pevm's performance.
#[inline(always)]
fn hash_determinisitic<T: Hash>(x: T) -> u64 {
    FxBuildHasher.hash_one(x)
}

// TODO: It would be nice if we could tie the different cases of
// memory locations & values at the type level, to prevent lots of
// matches & potentially dangerous mismatch mistakes.
#[derive(Debug, Clone)]
enum MemoryValue {
    Basic(AccountBasic),
    CodeHash(B256),
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
    // The account was self-destructed.
    SelfDestructed,
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
enum IncarnationStatus {
    ReadyToExecute,
    Executing,
    Executed,
    Validated,
    Aborting,
}

#[derive(PartialEq, Debug)]
struct TxStatus {
    incarnation: TxIncarnation,
    status: IncarnationStatus,
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

// Most memory locations only have one read origin. Lazy updated ones like
// the beneficiary balance, raw transfer senders & recipients, etc. have a
// list of lazy updates all the way to the first strict/absolute value.
type ReadOrigins = SmallVec<[ReadOrigin; 1]>;

// For validation: a list of read origins (previous transaction versions)
// for each read memory location.
type ReadSet = HashMap<MemoryLocationHash, ReadOrigins, BuildIdentityHasher>;

// The updates made by this transaction incarnation, which is applied
// to the multi-version data structure at the end of execution.
type WriteSet = Vec<(MemoryLocationHash, MemoryValue)>;

// A scheduled worker task
// TODO: Add more useful work when there are idle workers like near
// the end of block execution, while waiting for a huge blocking
// transaction to resolve, etc.
#[derive(Debug)]
enum Task {
    Execution(TxVersion),
    Validation(TxVersion),
}

bitflags! {
    struct FinishExecFlags: u8 {
        // Do we need to validate from this transaction?
        // The first and lazy transactions don't need validation. Note
        // that this is used to tune the min validation index in the
        // scheduler, meaning a [false] here will still be validated if
        // there was a lower transaction that has broken the preprocessed
        // dependency chain and returned [true]
        const NeedValidation = 0;
        // We need to validate from the next transaction if this execution
        // wrote to a new location.
        const WroteNewLocation = 1;
    }
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
pub use pevm::{execute_revm_sequential, Pevm, PevmError, PevmResult, PevmExecutionError};
mod scheduler;
mod storage;
pub use storage::{
    AccountBasic, BlockHashes, Bytecodes, ChainState, EvmAccount, EvmCode, InMemoryStorage,
    Storage, StorageWrapper,
};
mod vm;
pub use vm::{ExecutionError, PevmTxExecutionResult};

#[cfg(feature = "rpc-storage")]
pub use storage::RpcStorage;
