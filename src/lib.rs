//! Blazingly fast Block-STM implementation for EVM.

// TODO: Better types & API please

use revm::primitives::{AccountInfo, Address, U256};

// For simplicity, we first stop at the address & storage level. We
// can still break an address into smaller memory locations to
// minimize re-executions on "broad" state conflicts?
// TODO: It would be nice if we could tie the different cases of
// memory locations & values at the type level, to prevent lots of
// matches & potentially dangerous mismatch mistakes.
// TODO: Confirm that we're not missing anything, like bytecode.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MemoryLocation {
    Basic(Address),
    Storage((Address, U256)),
}

#[derive(Clone)]
enum MemoryValue {
    // TODO: Remove the Option here?
    Basic(Option<AccountInfo>),
    Storage(U256),
}

// The index of the transaction in the block.
type TxIdx = usize;

// The i-th time a transaction is re-executed, counting from 0.
type TxIncarnation = usize;

// BlockSTM maintains an in-memory multi-version data structure that
// stores for each memory location the latest value written per
// transaction, along with the associated transaction version. When a
// transaciton reads a memory location, it obtains from the
// multi-version data structure the value written to this location by
// the highest transaction that appears before it in the block, along
// with the associated version. For instance, tx5 would read the value
// written by tx3 even when tx6 has also written to it. If no previous
// transactions have written to a location, the value would be read
// from the storage state before block execution.
#[derive(Clone, Debug, PartialEq)]
struct TxVersion {
    tx_idx: TxIdx,
    tx_incarnation: TxIncarnation,
}

// The origin of a memory read. It could be from the live multi-version
// data structure or from storage (chain state before block execution).
#[derive(PartialEq)]
enum ReadOrigin {
    // The previous transaction version that wrote the value.
    MvMemory(TxVersion),
    Storage,
}

/// Errors when reading a memory location while executing BlockSTM.
/// TODO: Better name & elaboration
#[derive(Debug)]
pub enum ReadError {
    /// Memory location not found.
    NotFound,
    /// This memory location has been written by a lower transaction.
    BlockingIndex(usize),
    /// The stored memory value type doesn't match its location type.
    /// TODO: Handle this at the type level?
    InvalidMemoryLocationType,
}

// The memory locations needed to execute an incarnation.
// TODO: Properly type this
type ReadSet = Vec<(MemoryLocation, ReadOrigin)>;

// The updates made by this transaction incarnation, which is applied
// to the multi-version data structure at the end of execution.
// TODO: Properly type this
type WriteSet = Vec<(MemoryLocation, MemoryValue)>;

// TODO: Properly type this
type ExecutionTask = TxVersion;

// TODO: Properly type this
type ValidationTask = TxVersion;

// TODO: Properly type this
#[derive(Debug)]
enum Task {
    Execution(ExecutionTask),
    Validation(ValidationTask),
}

mod block_stm;
pub use block_stm::BlockSTM;
mod mv_memory;
mod scheduler;
mod storage;
pub use storage::Storage;
mod vm;
