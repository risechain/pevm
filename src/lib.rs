//! Blazingly fast Block-STM implementation for EVM.

// TODO: Better API & access control

use revm::primitives::{AccountInfo, Address, U256};

type TxIdx = usize;

// The i-th time a transaction is re-executed, counting from 0.
type TxIncarnation = usize;

// To support reads & writes by transactions that may execute
// concurrently, BlockSTM maintains an in-memory multi-version
// data structure that separately stores for each memory location
// the latest value written per transaction, along with the associated
// transaction version. When a transaciton reads a memory location,
// it obtains from the multi-version data structure the value written
// to this location by the highest transaction that appears before it
// in the transaction order, along with the associated version. For
// instance, tx5 would read the value written by tx3 even when tx6
// has also written to it. If no previous transactions have written
// to a location, the value would be read from the storage state
// before block execution.
#[derive(Clone, Debug, PartialEq)]
struct TxVersion {
    tx_idx: TxIdx,
    tx_incarnation: TxIncarnation,
}

// For simplicity, we first stop at the address & storage level. We
// can still break an address into smaller memory locations to
// minimize re-executions on "broad" state conflicts?
// TODO: Minor but it would be nice if we could tie the two
// different cases of basic & storage memory locations & values at the
// type level to prevent lots of matches & potential mismatch mistakes.
// TODO: Confirm that we're not missing anything, like bytecode.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MemoryLocation {
    Basic(Address),
    Storage((Address, U256)),
}

#[derive(Clone)]
enum MemoryValue {
    Basic(Option<AccountInfo>),
    Storage(U256),
}

// The memory locations read during the transaction incarnation, and
// the corresponding transaction version that wrote it.
#[derive(PartialEq)]
enum ReadOrigin {
    // The previous transaction version that wrote the value.
    MvMemory(TxVersion),
    Storage,
}

// TODO: Elaborate
pub(crate) enum ReadError {
    NotFound,
    BlockingIndex(usize),
    InvalidMemoryLocationType, // TODO: Elaborate
}

type ReadSet = Vec<(MemoryLocation, ReadOrigin)>;

// The updates made by this transaction incarnation, which is applied
// to shared memory (the multi-version data structure) at the end of
// execution.
type WriteSet = Vec<(MemoryLocation, MemoryValue)>;

// After an incarnation executes it needs to pass validation. The
// validation re-reads the read-set and compares the observed versions.
// A successful validation implies that the applied writes are still
// up-to-date. A failed validation means the incarnation must be
// aborted and the transaction is re-executed in a next one.
//
// The transaction order dictates that transactions must be committed in
// order, so a successful validation of an incarnation does not guarantee
// that it can be committed. This is beacause an abort and re-execution
// of an earlier transaction in the block might invalidate the incarnation
// read set and necessitate re-execution. Thus, when a transaction aborts,
// all higher transactions are scheduled for re-validation. The same
// incarnation may be validated multiple times, by different threads, and
// potentially in parallel, but BlockSTM ensures that only the first abort
// per version succeeds (the rest are ignored).
//
// Since transactions must be committed in order, BlockSTM prioritizes
// tasks (execution & validation) associated with lower-indexed transactions.

// TODO: Properly type these
type ExecutionTask = TxVersion;

// TODO: Properly type these
type ValidationTask = TxVersion;

#[derive(Debug)]
enum Task {
    Execution(ExecutionTask),
    Validation(ValidationTask),
}

mod block_stm;
pub use block_stm::BlockSTM;
mod mv_memory;
mod scheduler;
pub mod storage;
use storage::Storage;
pub mod examples;
mod vm;
