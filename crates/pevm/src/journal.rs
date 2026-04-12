//! Parallel EVM aware journal implementation.

use std::vec::Vec;

use revm::{
    Database,
    context::JournalTr,
    context::journal::{JournalCfg, warm_addresses::WarmAddresses},
    context_interface::journaled_state::entry::SelfdestructionRevertStatus,
    context_interface::{
        ErasedError,
        context::{SStoreResult, SelfDestructResult, StateLoad},
        journaled_state::{
            AccountInfoLoad, AccountLoad, JournalCheckpoint, JournalLoadErasedError,
            JournalLoadError, TransferError, account::JournaledAccountTr,
        },
    },
    primitives::{
        Address, AddressMap, AddressSet, B256, Bytes, HashSet, KECCAK_EMPTY, Log, LogData,
        PRECOMPILE3, StorageKey, StorageValue, U256,
        eip7708::{ETH_TRANSFER_LOG_ADDRESS, ETH_TRANSFER_LOG_TOPIC, SELFDESTRUCT_LOG_TOPIC},
        hardfork::SpecId,
        hash_map::Entry,
    },
    state::{Account, Bytecode, EvmState, EvmStorageSlot, TransientStorage},
};

use crate::{
    AccountBasic, BuildIdentityHasher, MemoryLocation, MemoryLocationHash, MemoryValue, WriteSet,
    hash_deterministic,
};

// ---------------------------------------------------------------------------
// pevm-owned journal entry type
// ---------------------------------------------------------------------------

/// Journal entry that records a single reversible state change.
///
/// Structurally identical to `revm::JournalEntry` but owned by pevm.
/// Every [`PevmJournal`] uses `Vec<PevmJournalEntry>` for its revert journal,
/// eliminating any runtime dependency on revm's concrete entry type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PevmJournalEntry {
    AccountWarmed {
        address: Address,
    },
    AccountDestroyed {
        had_balance: U256,
        address: Address,
        target: Address,
        destroyed_status: SelfdestructionRevertStatus,
    },
    AccountTouched {
        address: Address,
    },
    BalanceChange {
        old_balance: U256,
        address: Address,
    },
    BalanceTransfer {
        balance: U256,
        from: Address,
        to: Address,
    },
    NonceChange {
        address: Address,
        previous_nonce: u64,
    },
    NonceBump {
        address: Address,
    },
    AccountCreated {
        address: Address,
        is_created_globally: bool,
    },
    StorageChanged {
        key: StorageKey,
        had_value: StorageValue,
        address: Address,
    },
    StorageWarmed {
        key: StorageKey,
        address: Address,
    },
    TransientStorageChange {
        key: StorageKey,
        had_value: StorageValue,
        address: Address,
    },
    CodeChange {
        address: Address,
    },
}

impl PevmJournalEntry {
    pub(crate) fn revert(
        self,
        state: &mut EvmState,
        transient_storage: Option<&mut TransientStorage>,
        is_spurious_dragon_enabled: bool,
    ) {
        match self {
            Self::AccountWarmed { address } => {
                state.get_mut(&address).unwrap().mark_cold();
            }
            Self::AccountTouched { address } => {
                if is_spurious_dragon_enabled && address == PRECOMPILE3 {
                    return;
                }
                state.get_mut(&address).unwrap().unmark_touch();
            }
            Self::AccountDestroyed {
                address,
                target,
                destroyed_status,
                had_balance,
            } => {
                let account = state.get_mut(&address).unwrap();
                match destroyed_status {
                    SelfdestructionRevertStatus::GloballySelfdestroyed => {
                        account.unmark_selfdestruct();
                        account.unmark_selfdestructed_locally();
                    }
                    SelfdestructionRevertStatus::LocallySelfdestroyed => {
                        account.unmark_selfdestructed_locally();
                    }
                    SelfdestructionRevertStatus::RepeatedSelfdestruction => {}
                }
                account.info.balance += had_balance;
                if address != target {
                    state.get_mut(&target).unwrap().info.balance -= had_balance;
                }
            }
            Self::BalanceChange {
                address,
                old_balance,
            } => {
                state.get_mut(&address).unwrap().info.balance = old_balance;
            }
            Self::BalanceTransfer { from, to, balance } => {
                state.get_mut(&from).unwrap().info.balance += balance;
                state.get_mut(&to).unwrap().info.balance -= balance;
            }
            Self::NonceChange {
                address,
                previous_nonce,
            } => {
                state.get_mut(&address).unwrap().info.nonce = previous_nonce;
            }
            Self::NonceBump { address } => {
                let nonce = &mut state.get_mut(&address).unwrap().info.nonce;
                *nonce = nonce.saturating_sub(1);
            }
            Self::AccountCreated {
                address,
                is_created_globally,
            } => {
                let account = state.get_mut(&address).unwrap();
                account.unmark_created_locally();
                if is_created_globally {
                    account.unmark_created();
                }
                account.info.nonce = 0;
            }
            Self::StorageWarmed { address, key } => {
                state
                    .get_mut(&address)
                    .unwrap()
                    .storage
                    .get_mut(&key)
                    .unwrap()
                    .mark_cold();
            }
            Self::StorageChanged {
                address,
                key,
                had_value,
            } => {
                state
                    .get_mut(&address)
                    .unwrap()
                    .storage
                    .get_mut(&key)
                    .unwrap()
                    .present_value = had_value;
            }
            Self::TransientStorageChange {
                address,
                key,
                had_value,
            } => {
                let Some(ts) = transient_storage else { return };
                let tkey = (address, key);
                if had_value.is_zero() {
                    ts.remove(&tkey);
                } else {
                    ts.insert(tkey, had_value);
                }
            }
            Self::CodeChange { address } => {
                let acc = state.get_mut(&address).unwrap();
                acc.info.code_hash = KECCAK_EMPTY;
                acc.info.code = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// pevm-owned JournaledAccount
// ---------------------------------------------------------------------------

/// Wraps a mutable account reference together with the journal, database,
/// and write-set context needed to record state changes and build the write
/// set incrementally. Implements [`JournaledAccountTr`].
pub struct PevmJournaledAccount<'a, DB> {
    address: Address,
    /// `hash_deterministic(MemoryLocation::Basic(address))` — cached to avoid
    /// recomputing on every balance/nonce mutation within a single account access.
    basic_hash: MemoryLocationHash,
    account: &'a mut Account,
    journal: &'a mut Vec<PevmJournalEntry>,
    access_list: &'a AddressMap<HashSet<StorageKey>>,
    transaction_id: usize,
    db: &'a mut DB,
    // write-set context
    write_set: &'a mut WriteSet,
    new_bytecodes: &'a mut Vec<(B256, Bytecode)>,
    is_lazy: bool,
    from_hash: MemoryLocationHash,
    to_hash: Option<MemoryLocationHash>,
    tx_value: U256,
    is_eip161: bool,
}

impl<DB> std::fmt::Debug for PevmJournaledAccount<'_, DB> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PevmJournaledAccount")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl<'a, DB: Database> PevmJournaledAccount<'a, DB> {
    #[inline]
    #[allow(clippy::too_many_arguments, clippy::missing_const_for_fn)]
    pub(crate) fn new(
        address: Address,
        account: &'a mut Account,
        journal: &'a mut Vec<PevmJournalEntry>,
        db: &'a mut DB,
        access_list: &'a AddressMap<HashSet<StorageKey>>,
        transaction_id: usize,
        write_set: &'a mut WriteSet,
        new_bytecodes: &'a mut Vec<(B256, Bytecode)>,
        is_lazy: bool,
        from_hash: MemoryLocationHash,
        to_hash: Option<MemoryLocationHash>,
        tx_value: U256,
        is_eip161: bool,
    ) -> Self {
        Self {
            address,
            basic_hash: hash_deterministic(MemoryLocation::Basic(address)),
            account,
            journal,
            access_list,
            transaction_id,
            db,
            write_set,
            new_bytecodes,
            is_lazy,
            from_hash,
            to_hash,
            tx_value,
            is_eip161,
        }
    }

    /// Push the appropriate basic write set entry for this account.
    #[inline]
    fn push_basic_ws(&mut self) {
        let hash = self.basic_hash;
        let value = if self.is_lazy {
            if hash == self.from_hash {
                Some(MemoryValue::LazySender(
                    U256::MAX - self.account.info.balance,
                ))
            } else if Some(hash) == self.to_hash {
                Some(MemoryValue::LazyRecipient(self.tx_value))
            } else {
                None
            }
        } else if !self.is_eip161 || !self.account.is_empty() {
            Some(MemoryValue::Basic(AccountBasic {
                balance: self.account.info.balance,
                nonce: self.account.info.nonce,
            }))
        } else {
            None
        };
        if let Some(v) = value {
            self.write_set.push((hash, v));
        }
    }

    /// Load a storage slot, marking it warm if cold. Returns `ColdLoadSkipped`
    /// when the slot is cold and `skip_cold_load` is true.
    #[inline(never)]
    pub(crate) fn sload_concrete_error(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&mut EvmStorageSlot>, JournalLoadError<DB::Error>> {
        let is_newly_created = self.account.is_created();
        let (slot, is_cold) = match self.account.storage.entry(key) {
            Entry::Occupied(occ) => {
                let slot = occ.into_mut();
                let mut is_cold = false;
                if slot.is_cold_transaction_id(self.transaction_id) {
                    is_cold = self
                        .access_list
                        .get(&self.address)
                        .and_then(|v| v.get(&key))
                        .is_none();
                    if is_cold && skip_cold_load {
                        return Err(JournalLoadError::ColdLoadSkipped);
                    }
                }
                slot.mark_warm_with_transaction_id(self.transaction_id);
                (slot, is_cold)
            }
            Entry::Vacant(vac) => {
                let is_cold = self
                    .access_list
                    .get(&self.address)
                    .and_then(|v| v.get(&key))
                    .is_none();
                if is_cold && skip_cold_load {
                    return Err(JournalLoadError::ColdLoadSkipped);
                }
                let value = if is_newly_created {
                    StorageValue::ZERO
                } else {
                    self.db.storage(self.address, key)?
                };
                let slot = vac.insert(EvmStorageSlot::new(value, self.transaction_id));
                (slot, is_cold)
            }
        };
        if is_cold {
            self.journal.push(PevmJournalEntry::StorageWarmed {
                address: self.address,
                key,
            });
        }
        Ok(StateLoad::new(slot, is_cold))
    }

    /// Store a value into a storage slot.
    #[inline]
    pub(crate) fn sstore_concrete_error(
        &mut self,
        key: StorageKey,
        new: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadError<DB::Error>> {
        self.touch();
        let slot = self.sload_concrete_error(key, skip_cold_load)?;
        let ret = Ok(StateLoad::new(
            SStoreResult {
                original_value: slot.original_value(),
                present_value: slot.present_value(),
                new_value: new,
            },
            slot.is_cold,
        ));
        if slot.present_value != new {
            let previous_value = slot.present_value;
            slot.data.present_value = new;
            self.journal.push(PevmJournalEntry::StorageChanged {
                address: self.address,
                key,
                had_value: previous_value,
            });
            self.write_set.push((
                hash_deterministic(MemoryLocation::Storage(self.address, key)),
                MemoryValue::Storage(new),
            ));
        }
        ret
    }

    /// Load the account's bytecode, fetching from the database if needed.
    #[inline]
    pub(crate) fn load_code_preserve_error(
        &mut self,
    ) -> Result<&Bytecode, JournalLoadError<DB::Error>> {
        if self.account.info.code.is_none() {
            let hash = self.account.info.code_hash;
            let code = if hash == KECCAK_EMPTY {
                Bytecode::default()
            } else {
                self.db.code_by_hash(hash)?
            };
            self.account.info.code = Some(code);
        }
        Ok(self.account.info.code.as_ref().unwrap())
    }
}

impl<DB: Database> JournaledAccountTr for PevmJournaledAccount<'_, DB> {
    fn account(&self) -> &Account {
        self.account
    }

    fn balance(&self) -> &U256 {
        &self.account.info.balance
    }

    fn nonce(&self) -> u64 {
        self.account.info.nonce
    }

    fn code_hash(&self) -> &B256 {
        &self.account.info.code_hash
    }

    fn code(&self) -> Option<&Bytecode> {
        self.account.info.code.as_ref()
    }

    #[inline]
    fn touch(&mut self) {
        if !self.account.status.is_touched() {
            self.account.mark_touch();
            self.journal.push(PevmJournalEntry::AccountTouched {
                address: self.address,
            });
        }
    }

    fn unsafe_mark_cold(&mut self) {
        self.account.mark_cold();
    }

    #[inline]
    fn set_balance(&mut self, balance: U256) {
        self.touch();
        if self.account.info.balance != balance {
            self.journal.push(PevmJournalEntry::BalanceChange {
                address: self.address,
                old_balance: self.account.info.balance,
            });
            self.account.info.set_balance(balance);
            self.push_basic_ws();
        }
    }

    #[inline]
    fn incr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(new_balance) = self.account.info.balance.checked_add(balance) else {
            return false;
        };
        self.set_balance(new_balance);
        true
    }

    #[inline]
    fn decr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(new_balance) = self.account.info.balance.checked_sub(balance) else {
            return false;
        };
        self.set_balance(new_balance);
        true
    }

    #[inline]
    fn bump_nonce(&mut self) -> bool {
        self.touch();
        let Some(nonce) = self.account.info.nonce.checked_add(1) else {
            return false;
        };
        self.account.info.set_nonce(nonce);
        self.journal.push(PevmJournalEntry::NonceBump {
            address: self.address,
        });
        self.push_basic_ws();
        true
    }

    #[inline]
    fn set_nonce(&mut self, nonce: u64) {
        self.touch();
        let previous_nonce = self.account.info.nonce;
        self.account.info.set_nonce(nonce);
        self.journal.push(PevmJournalEntry::NonceChange {
            address: self.address,
            previous_nonce,
        });
        self.push_basic_ws();
    }

    fn unsafe_set_nonce(&mut self, nonce: u64) {
        self.account.info.set_nonce(nonce);
    }

    #[inline]
    fn set_code(&mut self, code_hash: B256, code: Bytecode) {
        self.touch();
        self.account.info.set_code_and_hash(code, code_hash);
        self.journal.push(PevmJournalEntry::CodeChange {
            address: self.address,
        });
        self.push_basic_ws();
        if code_hash != KECCAK_EMPTY {
            self.write_set.push((
                hash_deterministic(MemoryLocation::CodeHash(self.address)),
                MemoryValue::CodeHash(code_hash),
            ));
            if let Some(c) = self.account.info.code.clone() {
                self.new_bytecodes.push((code_hash, c));
            }
        }
    }

    fn set_code_and_hash_slow(&mut self, code: Bytecode) {
        let code_hash = code.hash_slow();
        self.set_code(code_hash, code);
    }

    fn delegate(&mut self, address: Address) {
        let (bytecode, hash) = if address.is_zero() {
            (Bytecode::default(), KECCAK_EMPTY)
        } else {
            let bytecode = Bytecode::new_eip7702(address);
            let hash = bytecode.hash_slow();
            (bytecode, hash)
        };
        self.touch();
        self.set_code(hash, bytecode);
        self.bump_nonce();
    }

    fn sload(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&mut EvmStorageSlot>, JournalLoadErasedError> {
        self.sload_concrete_error(key, skip_cold_load)
            .map_err(|e| e.map(ErasedError::new))
    }

    fn sstore(
        &mut self,
        key: StorageKey,
        new: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadErasedError> {
        self.sstore_concrete_error(key, new, skip_cold_load)
            .map_err(|e| e.map(ErasedError::new))
    }

    fn load_code(&mut self) -> Result<&Bytecode, JournalLoadErasedError> {
        self.load_code_preserve_error()
            .map_err(|e| e.map(ErasedError::new))
    }
}

// ---------------------------------------------------------------------------
// PevmJournal
// ---------------------------------------------------------------------------

/// A [`JournalTr`] implementation for pevm's parallel EVM execution.
///
/// All EVM journal state is held directly as struct fields — no sub-structs,
/// no `JournalInner`, no `call_inner` trampoline.  The revert journal uses
/// [`PevmJournalEntry`], a pevm-owned type, instead of revm's `JournalEntry`.
#[derive(Debug)]
pub struct PevmJournal<DB: Database> {
    // ---- Inlined EVM journal state ----
    pub(crate) state: EvmState,
    pub(crate) transient_storage: TransientStorage,
    pub(crate) logs: Vec<Log>,
    pub(crate) depth: usize,
    pub(crate) journal: Vec<PevmJournalEntry>,
    pub(crate) transaction_id: usize,
    pub(crate) cfg: JournalCfg,
    pub(crate) warm_addresses: WarmAddresses,
    pub(crate) selfdestructed_addresses: Vec<Address>,
    // ---- Database ----
    pub(crate) database: DB,
    // ---- pevm-specific fields ----
    is_lazy: bool,
    is_eip161: bool,
    /// Address of the transaction sender.  Stored alongside `from_hash` so
    /// that `push_basic_ws_for` can skip `hash_deterministic` via a cheap
    /// 20-byte address comparison for the most-frequently-pushed account.
    from_address: Address,
    from_hash: MemoryLocationHash,
    /// Address of the transaction recipient (None for contract-creation txs).
    to_address: Option<Address>,
    to_hash: Option<MemoryLocationHash>,
    tx_value: U256,
    write_set_checkpoints: Vec<usize>,
    pending_write_set: WriteSet,
    pending_new_bytecodes: Vec<(B256, Bytecode)>,
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

impl<DB: Database> PevmJournal<DB> {
    /// Construct a [`PevmJournal`] from an already-initialised
    /// `revm::context::journal::Journal<DB>`, transferring its configuration
    /// (spec, precompiles, warm addresses, …).
    ///
    /// The revert journal inside the source `Journal` is discarded: it is
    /// always empty at the point where `build_evm` creates the context.
    pub fn from_journal(journal: revm::context::journal::Journal<DB>) -> Self {
        let revm::context::journal::Journal { inner, database } = journal;
        Self {
            state: inner.state,
            transient_storage: inner.transient_storage,
            logs: inner.logs,
            depth: inner.depth,
            journal: Vec::new(), // inner.journal is empty at construction time
            transaction_id: inner.transaction_id,
            cfg: inner.cfg,
            warm_addresses: inner.warm_addresses,
            selfdestructed_addresses: inner.selfdestructed_addresses,
            database,
            is_lazy: false,
            is_eip161: false,
            from_address: Address::ZERO,
            from_hash: 0,
            to_address: None,
            to_hash: None,
            tx_value: U256::ZERO,
            write_set_checkpoints: Vec::new(),
            pending_write_set: WriteSet::new(),
            pending_new_bytecodes: Vec::new(),
        }
    }

    /// Configure pevm-specific context before executing a transaction.
    pub(crate) fn set_pevm_tx(
        &mut self,
        is_lazy: bool,
        is_eip161: bool,
        from_address: Address,
        from_hash: MemoryLocationHash,
        to_address: Option<Address>,
        to_hash: Option<MemoryLocationHash>,
        tx_value: U256,
    ) {
        self.is_lazy = is_lazy;
        self.is_eip161 = is_eip161;
        self.from_address = from_address;
        self.from_hash = from_hash;
        self.to_address = to_address;
        self.to_hash = to_hash;
        self.tx_value = tx_value;
    }

    /// Take the write-set and newly-deployed bytecodes produced by the most
    /// recent transaction execution.  Must be called **before** [`JournalTr::finalize`].
    ///
    /// Deduplicates the write-set in place (last write per location wins) before
    /// returning it.  Clearing the write-set is left to the subsequent `finalize()`
    /// call so that the field is always empty at the start of the next transaction.
    pub(crate) fn take_write_set(&mut self) -> (WriteSet, Vec<(B256, Bytecode)>) {
        // Dedup write set: keep last entry per hash (last write wins).
        let n = self.pending_write_set.len();
        if n > 1 {
            if n <= 8 {
                // Small write set (fits in SmallVec inline storage): mark-and-compact
                // in-place with O(n²) scan — no heap allocation.
                let mut keep: u16 = (1u16 << n) - 1;
                for i in 0..(n - 1) {
                    if keep & (1u16 << i) == 0 {
                        continue;
                    }
                    let h = self.pending_write_set[i].0;
                    for j in (i + 1)..n {
                        if self.pending_write_set[j].0 == h {
                            keep &= !(1u16 << i);
                            break;
                        }
                    }
                }
                if keep != (1u16 << n) - 1 {
                    let mut write = 0usize;
                    for read in 0..n {
                        if keep & (1u16 << read) != 0 {
                            self.pending_write_set.swap(write, read);
                            write += 1;
                        }
                    }
                    self.pending_write_set.truncate(write);
                }
            } else {
                // Larger write set: reverse so first occurrence of each hash (scanning
                // forward) is the last write, then retain unique hashes.
                self.pending_write_set.reverse();
                let mut seen =
                    HashSet::with_capacity_and_hasher(n, BuildIdentityHasher::default());
                self.pending_write_set.retain(|(h, _)| seen.insert(*h));
            }
        }
        (
            std::mem::take(&mut self.pending_write_set),
            std::mem::take(&mut self.pending_new_bytecodes),
        )
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

impl<DB: Database> PevmJournal<DB> {
    /// Resolve the `Basic` location hash for `address`, reusing the pre-stored
    /// `from_hash` / `to_hash` via a cheap address comparison when possible.
    #[inline]
    fn basic_hash_for(&self, address: Address) -> MemoryLocationHash {
        if address == self.from_address {
            self.from_hash
        } else if self.to_address == Some(address) {
            // SAFETY: to_hash is Some whenever to_address is Some
            self.to_hash.unwrap()
        } else {
            hash_deterministic(MemoryLocation::Basic(address))
        }
    }

    /// Push the appropriate basic write set entry given a pre-computed hash
    /// and already-known account data.  Callers that hold the mutable account
    /// borrow should save `balance`, `nonce`, and `code_hash` before the NLL
    /// borrow is dropped, then call this to avoid a redundant `state.get()`.
    #[inline]
    fn push_basic_ws_with_hash(
        &mut self,
        hash: MemoryLocationHash,
        balance: U256,
        nonce: u64,
        code_hash: B256,
    ) {
        let value = if self.is_lazy {
            if hash == self.from_hash {
                Some(MemoryValue::LazySender(U256::MAX - balance))
            } else if Some(hash) == self.to_hash {
                Some(MemoryValue::LazyRecipient(self.tx_value))
            } else {
                None
            }
        } else if !self.is_eip161 || balance != U256::ZERO || nonce != 0 || code_hash != KECCAK_EMPTY {
            Some(MemoryValue::Basic(AccountBasic { balance, nonce }))
        } else {
            None
        };
        if let Some(v) = value {
            self.pending_write_set.push((hash, v));
        }
    }

    /// Push the appropriate basic write set entry for `address` using current
    /// account state.  Prefer [`push_basic_ws_with_hash`] when the account data
    /// is already available to avoid a redundant `state.get()`.
    #[inline]
    fn push_basic_ws_for(&mut self, address: Address) {
        let hash = self.basic_hash_for(address);
        let account = self.state.get(&address).expect("loaded");
        self.push_basic_ws_with_hash(hash, account.info.balance, account.info.nonce, account.info.code_hash);
    }

    /// Core account-loading primitive.
    ///
    /// Mirrors `JournalInner::load_account_mut_optional` but returns only
    /// `is_cold: bool` (not a `JournaledAccount`), so callers can borrow
    /// specific fields of `self` afterwards without lifetime conflicts.
    fn load_account_optional_cold(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<bool, JournalLoadError<DB::Error>> {
        let is_cold = match self.state.entry(address) {
            Entry::Occupied(entry) => {
                let account = entry.into_mut();
                let mut is_cold = account.is_cold_transaction_id(self.transaction_id);
                if is_cold {
                    is_cold = self
                        .warm_addresses
                        .check_is_cold(&address, skip_cold_load)?;
                    account.mark_warm_with_transaction_id(self.transaction_id);
                    if account.is_selfdestructed_locally() {
                        account.selfdestruct();
                        account.unmark_selfdestructed_locally();
                    }
                    *account.original_info = account.info.clone();
                    account.unmark_created_locally();
                    self.journal
                        .push(PevmJournalEntry::AccountWarmed { address });
                }
                is_cold
            }
            Entry::Vacant(vac) => {
                let is_cold = self
                    .warm_addresses
                    .check_is_cold(&address, skip_cold_load)?;
                let account = if let Some(info) = self.database.basic(address)? {
                    let mut account: Account = info.into();
                    account.transaction_id = self.transaction_id;
                    account
                } else {
                    Account::new_not_existing(self.transaction_id)
                };
                if is_cold {
                    self.journal
                        .push(PevmJournalEntry::AccountWarmed { address });
                }
                vac.insert(account);
                is_cold
            }
        };
        Ok(is_cold)
    }

    // --- EIP-7708 helpers ---

    #[inline]
    fn eip7708_transfer_log(&mut self, from: Address, to: Address, balance: U256) {
        if !self.cfg.spec.is_enabled_in(SpecId::AMSTERDAM)
            || self.cfg.eip7708_disabled
            || balance.is_zero()
        {
            return;
        }
        let topics = std::vec![
            ETH_TRANSFER_LOG_TOPIC,
            B256::left_padding_from(from.as_slice()),
            B256::left_padding_from(to.as_slice()),
        ];
        let data = Bytes::copy_from_slice(&balance.to_be_bytes::<32>());
        self.logs.push(Log {
            address: ETH_TRANSFER_LOG_ADDRESS,
            data: LogData::new(topics, data).expect("3 topics is valid"),
        });
    }

    #[inline]
    fn eip7708_selfdestruct_to_self_log(&mut self, address: Address, balance: U256) {
        if !self.cfg.spec.is_enabled_in(SpecId::AMSTERDAM)
            || self.cfg.eip7708_disabled
            || balance.is_zero()
        {
            return;
        }
        let topics = std::vec![
            SELFDESTRUCT_LOG_TOPIC,
            B256::left_padding_from(address.as_slice()),
        ];
        let data = Bytes::copy_from_slice(&balance.to_be_bytes::<32>());
        self.logs.push(Log {
            address: ETH_TRANSFER_LOG_ADDRESS,
            data: LogData::new(topics, data).expect("2 topics is valid"),
        });
    }

    fn eip7708_emit_selfdestruct_remaining_balance_logs(&mut self) {
        if !self.cfg.spec.is_enabled_in(SpecId::AMSTERDAM)
            || self.cfg.eip7708_disabled
            || self.cfg.eip7708_delayed_burn_disabled
        {
            return;
        }
        let mut addresses_with_balance: Vec<(Address, U256)> = self
            .selfdestructed_addresses
            .iter()
            .filter_map(|address| {
                self.state
                    .get(address)
                    .filter(|account| !account.info.balance.is_zero())
                    .map(|account| (*address, account.info.balance))
            })
            .collect();
        addresses_with_balance.sort_by_key(|(addr, _)| *addr);
        for (address, balance) in addresses_with_balance {
            self.eip7708_selfdestruct_to_self_log(address, balance);
        }
    }
}

// ---------------------------------------------------------------------------
// JournalTr implementation
// ---------------------------------------------------------------------------

impl<DB: Database> JournalTr for PevmJournal<DB> {
    type Database = DB;
    type State = EvmState;
    type JournaledAccount<'a>
        = PevmJournaledAccount<'a, DB>
    where
        DB: 'a;

    fn new(database: DB) -> Self {
        Self {
            state: EvmState::default(),
            transient_storage: TransientStorage::default(),
            logs: Vec::new(),
            depth: 0,
            journal: Vec::new(),
            transaction_id: 0,
            cfg: JournalCfg::default(),
            warm_addresses: WarmAddresses::new(),
            selfdestructed_addresses: Vec::new(),
            database,
            is_lazy: false,
            is_eip161: false,
            from_address: Address::ZERO,
            from_hash: 0,
            to_address: None,
            to_hash: None,
            tx_value: U256::ZERO,
            write_set_checkpoints: Vec::new(),
            pending_write_set: WriteSet::new(),
            pending_new_bytecodes: Vec::new(),
        }
    }

    fn clear(&mut self) {
        // Reset all state without calling finalize() — finalize() deduplicates
        // pending_write_set and normalises the EVM state, both of which are wasted
        // work when we're about to discard everything anyway (e.g. before a retry).
        self.pending_write_set.clear();
        self.pending_new_bytecodes.clear();
        self.write_set_checkpoints.clear();
        self.warm_addresses.clear_coinbase_and_access_list();
        self.selfdestructed_addresses.clear();
        self.state = EvmState::default();
        self.logs.clear();
        self.transient_storage.clear();
        self.journal.clear();
        self.depth = 0;
        self.transaction_id = 0;
    }

    fn db(&self) -> &DB {
        &self.database
    }

    fn db_mut(&mut self) -> &mut DB {
        &mut self.database
    }

    fn sload_skip_cold_load(
        &mut self,
        address: Address,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<StorageValue>, JournalLoadError<DB::Error>> {
        let account = self
            .state
            .get_mut(&address)
            .ok_or(JournalLoadError::ColdLoadSkipped)?;
        let mut ja = PevmJournaledAccount::new(
            address,
            account,
            &mut self.journal,
            &mut self.database,
            self.warm_addresses.access_list(),
            self.transaction_id,
            &mut self.pending_write_set,
            &mut self.pending_new_bytecodes,
            self.is_lazy,
            self.from_hash,
            self.to_hash,
            self.tx_value,
            self.is_eip161,
        );
        ja.sload_concrete_error(key, skip_cold_load)
            .map(|s| s.map(|s| s.present_value))
    }

    fn sstore_skip_cold_load(
        &mut self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadError<DB::Error>> {
        let account = self
            .state
            .get_mut(&address)
            .ok_or(JournalLoadError::ColdLoadSkipped)?;
        let mut ja = PevmJournaledAccount::new(
            address,
            account,
            &mut self.journal,
            &mut self.database,
            self.warm_addresses.access_list(),
            self.transaction_id,
            &mut self.pending_write_set,
            &mut self.pending_new_bytecodes,
            self.is_lazy,
            self.from_hash,
            self.to_hash,
            self.tx_value,
            self.is_eip161,
        );
        ja.sstore_concrete_error(key, value, skip_cold_load)
    }

    fn tload(&mut self, address: Address, key: StorageKey) -> StorageValue {
        self.transient_storage
            .get(&(address, key))
            .copied()
            .unwrap_or_default()
    }

    fn tstore(&mut self, address: Address, key: StorageKey, value: StorageValue) {
        let had_value = if value.is_zero() {
            self.transient_storage.remove(&(address, key))
        } else {
            let prev = self
                .transient_storage
                .insert((address, key), value)
                .unwrap_or_default();
            (prev != value).then_some(prev)
        };
        if let Some(had_value) = had_value {
            self.journal.push(PevmJournalEntry::TransientStorageChange {
                address,
                key,
                had_value,
            });
        }
    }

    fn log(&mut self, log: Log) {
        self.logs.push(log)
    }

    fn take_logs(&mut self) -> Vec<Log> {
        self.eip7708_emit_selfdestruct_remaining_balance_logs();
        std::mem::take(&mut self.logs)
    }

    fn logs(&self) -> &[Log] {
        &self.logs
    }

    fn selfdestruct(
        &mut self,
        address: Address,
        target: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SelfDestructResult>, JournalLoadError<DB::Error>> {
        let spec = self.cfg.spec;
        let is_cold = self.load_account_optional_cold(target, skip_cold_load)?;
        let is_empty = self
            .state
            .get(&target)
            .expect("loaded")
            .state_clear_aware_is_empty(spec);

        if address != target {
            let acc_balance = self.state.get(&address).expect("loaded").info.balance;
            let target_account = self.state.get_mut(&target).expect("loaded");
            if !target_account.is_touched() {
                self.journal
                    .push(PevmJournalEntry::AccountTouched { address: target });
                target_account.mark_touch();
            }
            target_account.info.balance += acc_balance;
        }

        let acc = self.state.get_mut(&address).expect("loaded");
        let balance = acc.info.balance;

        let destroyed_status = if !acc.is_selfdestructed() {
            SelfdestructionRevertStatus::GloballySelfdestroyed
        } else if !acc.is_selfdestructed_locally() {
            SelfdestructionRevertStatus::LocallySelfdestroyed
        } else {
            SelfdestructionRevertStatus::RepeatedSelfdestruction
        };

        let is_cancun_enabled = spec.is_enabled_in(SpecId::CANCUN);

        let journal_entry = if acc.is_created_locally() || !is_cancun_enabled {
            if destroyed_status == SelfdestructionRevertStatus::GloballySelfdestroyed
                && !self.cfg.eip7708_delayed_burn_disabled
            {
                self.selfdestructed_addresses.push(address);
            }
            acc.mark_selfdestructed_locally();
            acc.info.balance = U256::ZERO;
            // acc no longer used: NLL releases the self.state borrow here
            if address == target {
                self.eip7708_selfdestruct_to_self_log(address, balance);
            } else {
                self.eip7708_transfer_log(address, target, balance);
            }
            Some(PevmJournalEntry::AccountDestroyed {
                address,
                target,
                destroyed_status,
                had_balance: balance,
            })
        } else if address != target {
            acc.info.balance = U256::ZERO;
            // acc no longer used: NLL releases the self.state borrow here
            self.eip7708_transfer_log(address, target, balance);
            Some(PevmJournalEntry::BalanceTransfer {
                from: address,
                to: target,
                balance,
            })
        } else {
            None
        };

        // Push write set entries based on what happened, while journal_entry is
        // still a shared reference (before consuming it into the journal).
        match (&journal_entry, destroyed_status) {
            (
                Some(PevmJournalEntry::AccountDestroyed {
                    target: jt,
                    had_balance,
                    ..
                }),
                ds,
            ) => {
                // For non-repeated destructs, the account is (or remains) globally
                // selfdestructed — record it in the write set.
                if ds != SelfdestructionRevertStatus::RepeatedSelfdestruction {
                    self.pending_write_set.push((
                        hash_deterministic(MemoryLocation::CodeHash(address)),
                        MemoryValue::SelfDestructed,
                    ));
                }
                // If balance was sent to a different target, track target's new balance.
                if *jt != address && !had_balance.is_zero() {
                    self.push_basic_ws_for(*jt);
                }
            }
            (Some(PevmJournalEntry::BalanceTransfer { balance: b, .. }), _) if !b.is_zero() => {
                // Cancun+ non-locally-created: balance moved without true selfdestruct.
                self.push_basic_ws_for(address);
                self.push_basic_ws_for(target);
            }
            _ => {}
        }

        if let Some(entry) = journal_entry {
            self.journal.push(entry);
        }

        Ok(StateLoad::new(
            SelfDestructResult {
                had_value: !balance.is_zero(),
                target_exists: !is_empty,
                previously_destroyed: destroyed_status
                    == SelfdestructionRevertStatus::RepeatedSelfdestruction,
            },
            is_cold,
        ))
    }

    fn warm_access_list(&mut self, access_list: AddressMap<HashSet<StorageKey>>) {
        self.warm_addresses.set_access_list(access_list)
    }

    fn warm_coinbase_account(&mut self, address: Address) {
        self.warm_addresses.set_coinbase(address)
    }

    fn warm_precompiles(&mut self, addresses: AddressSet) {
        self.warm_addresses.set_precompile_addresses(addresses)
    }

    fn precompile_addresses(&self) -> &AddressSet {
        self.warm_addresses.precompiles()
    }

    fn set_spec_id(&mut self, spec_id: SpecId) {
        self.cfg.spec = spec_id
    }

    fn set_eip7708_config(&mut self, disabled: bool, delayed_burn_disabled: bool) {
        self.cfg.eip7708_disabled = disabled;
        self.cfg.eip7708_delayed_burn_disabled = delayed_burn_disabled;
    }

    fn touch_account(&mut self, address: Address) {
        if let Some(account) = self.state.get_mut(&address)
            && !account.is_touched()
        {
            self.journal
                .push(PevmJournalEntry::AccountTouched { address });
            account.mark_touch();
        }
    }

    fn transfer(
        &mut self,
        from: Address,
        to: Address,
        balance: U256,
    ) -> Result<Option<TransferError>, DB::Error> {
        self.load_account_optional_cold(from, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        self.load_account_optional_cold(to, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        Ok(self.transfer_loaded(from, to, balance))
    }

    fn transfer_loaded(
        &mut self,
        from: Address,
        to: Address,
        balance: U256,
    ) -> Option<TransferError> {
        if from == to {
            let from_balance = self.state.get_mut(&to).unwrap().info.balance;
            return (balance > from_balance).then_some(TransferError::OutOfFunds);
        }
        if balance.is_zero() {
            let to_acc = self.state.get_mut(&to).unwrap();
            if !to_acc.is_touched() {
                self.journal
                    .push(PevmJournalEntry::AccountTouched { address: to });
                to_acc.mark_touch();
            }
            // In lazy mode the recipient is loaded with a mock zero balance (VmDb
            // returns None for lazy recipients).  Without a write-set entry the
            // recipient stays at balance=0/nonce=0 in the EVM result, which EIP-161
            // turns into None — erasing an account that may have a real storage
            // balance.  Push LazyRecipient(0) so that pevm.rs evaluates the real
            // storage balance even when no ETH was transferred.
            if self.is_lazy {
                let to_balance = to_acc.info.balance;
                let to_nonce = to_acc.info.nonce;
                let to_code_hash = to_acc.info.code_hash;
                // to_acc borrow released by NLL here
                let to_hash = self.basic_hash_for(to);
                self.push_basic_ws_with_hash(to_hash, to_balance, to_nonce, to_code_hash);
            }
            return None;
        }
        let from_account = self.state.get_mut(&from).unwrap();
        if !from_account.is_touched() {
            self.journal
                .push(PevmJournalEntry::AccountTouched { address: from });
            from_account.mark_touch();
        }
        let from_balance = from_account.info.balance;
        let Some(from_balance_new) = from_balance.checked_sub(balance) else {
            return Some(TransferError::OutOfFunds);
        };
        from_account.info.balance = from_balance_new;
        let from_nonce = from_account.info.nonce;
        let from_code_hash = from_account.info.code_hash;
        // from_account no longer used: NLL releases self.state borrow here
        let to_account = self.state.get_mut(&to).unwrap();
        if !to_account.is_touched() {
            self.journal
                .push(PevmJournalEntry::AccountTouched { address: to });
            to_account.mark_touch();
        }
        let to_balance = to_account.info.balance;
        let Some(to_balance_new) = to_balance.checked_add(balance) else {
            return Some(TransferError::OverflowPayment);
        };
        to_account.info.balance = to_balance_new;
        let to_nonce = to_account.info.nonce;
        let to_code_hash = to_account.info.code_hash;
        // to_account no longer used: NLL releases self.state borrow here
        self.journal
            .push(PevmJournalEntry::BalanceTransfer { from, to, balance });
        self.eip7708_transfer_log(from, to, balance);
        let from_hash = self.basic_hash_for(from);
        self.push_basic_ws_with_hash(from_hash, from_balance_new, from_nonce, from_code_hash);
        let to_hash = self.basic_hash_for(to);
        self.push_basic_ws_with_hash(to_hash, to_balance_new, to_nonce, to_code_hash);
        None
    }

    #[allow(deprecated)]
    fn caller_accounting_journal_entry(
        &mut self,
        address: Address,
        old_balance: U256,
        bump_nonce: bool,
    ) {
        self.journal.push(PevmJournalEntry::BalanceChange {
            address,
            old_balance,
        });
        self.journal
            .push(PevmJournalEntry::AccountTouched { address });
        if bump_nonce {
            self.journal.push(PevmJournalEntry::NonceBump { address });
        }
        self.push_basic_ws_for(address);
    }

    fn balance_incr(&mut self, address: Address, balance: U256) -> Result<(), DB::Error> {
        self.load_account_optional_cold(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        let account = self.state.get_mut(&address).expect("loaded");
        if !account.is_touched() {
            self.journal
                .push(PevmJournalEntry::AccountTouched { address });
            account.mark_touch();
        }
        let mut ws_data: Option<(U256, u64, B256)> = None;
        if let Some(new_balance) = account.info.balance.checked_add(balance)
            && account.info.balance != new_balance
        {
            let old_balance = account.info.balance;
            self.journal.push(PevmJournalEntry::BalanceChange {
                address,
                old_balance,
            });
            account.info.set_balance(new_balance);
            ws_data = Some((new_balance, account.info.nonce, account.info.code_hash));
        }
        // account borrow released by NLL here
        if let Some((balance, nonce, code_hash)) = ws_data {
            let hash = self.basic_hash_for(address);
            self.push_basic_ws_with_hash(hash, balance, nonce, code_hash);
        }
        Ok(())
    }

    #[allow(deprecated)]
    fn nonce_bump_journal_entry(&mut self, address: Address) {
        self.journal.push(PevmJournalEntry::NonceBump { address });
        self.push_basic_ws_for(address);
    }

    fn load_account(&mut self, address: Address) -> Result<StateLoad<&Account>, DB::Error> {
        let is_cold = self
            .load_account_optional_cold(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        let account = self.state.get(&address).expect("loaded");
        Ok(StateLoad::new(account, is_cold))
    }

    fn load_account_with_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&Account>, DB::Error> {
        let is_cold = self
            .load_account_optional_cold(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        // Load code if not yet present.
        let account = self.state.get_mut(&address).expect("loaded");
        if account.info.code.is_none() {
            let hash = account.info.code_hash;
            let code = if hash == KECCAK_EMPTY {
                Bytecode::default()
            } else {
                self.database.code_by_hash(hash)?
            };
            account.info.code = Some(code);
        }
        let account = self.state.get(&address).expect("loaded");
        Ok(StateLoad::new(account, is_cold))
    }

    fn load_account_delegated(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<AccountLoad>, DB::Error> {
        let spec = self.cfg.spec;
        let is_eip7702 = spec.is_enabled_in(SpecId::PRAGUE);
        let is_cold = self
            .load_account_optional_cold(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        if is_eip7702 {
            // Load code so we can check for EIP-7702 delegation designation.
            let account = self.state.get_mut(&address).expect("loaded");
            if account.info.code.is_none() {
                let hash = account.info.code_hash;
                let code = if hash == KECCAK_EMPTY {
                    Bytecode::default()
                } else {
                    self.database.code_by_hash(hash)?
                };
                account.info.code = Some(code);
            }
        }
        let account = self.state.get(&address).expect("loaded");
        let is_empty = account.state_clear_aware_is_empty(spec);
        let mut account_load = StateLoad::new(
            AccountLoad {
                is_delegate_account_cold: None,
                is_empty,
            },
            is_cold,
        );
        // If EIP-7702 is enabled and account has a delegation, load the delegate.
        if is_eip7702
            && let Some(delegate_addr) = account
                .info
                .code
                .as_ref()
                .and_then(Bytecode::eip7702_address)
        {
            let delegate_is_cold = self
                .load_account_optional_cold(delegate_addr, false)
                .map_err(JournalLoadError::unwrap_db_error)?;
            // Load code for the delegated account too.
            let delegate_account = self.state.get_mut(&delegate_addr).expect("loaded");
            if delegate_account.info.code.is_none() {
                let hash = delegate_account.info.code_hash;
                let code = if hash == KECCAK_EMPTY {
                    Bytecode::default()
                } else {
                    self.database.code_by_hash(hash)?
                };
                delegate_account.info.code = Some(code);
            }
            account_load.data.is_delegate_account_cold = Some(delegate_is_cold);
        }
        Ok(account_load)
    }

    fn load_account_mut_skip_cold_load(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<Self::JournaledAccount<'_>>, DB::Error> {
        let is_cold = self
            .load_account_optional_cold(address, skip_cold_load)
            .map_err(JournalLoadError::unwrap_db_error)?;
        let account = self.state.get_mut(&address).expect("loaded");
        Ok(StateLoad::new(
            PevmJournaledAccount::new(
                address,
                account,
                &mut self.journal,
                &mut self.database,
                self.warm_addresses.access_list(),
                self.transaction_id,
                &mut self.pending_write_set,
                &mut self.pending_new_bytecodes,
                self.is_lazy,
                self.from_hash,
                self.to_hash,
                self.tx_value,
                self.is_eip161,
            ),
            is_cold,
        ))
    }

    fn load_account_mut_optional_code(
        &mut self,
        address: Address,
        load_code: bool,
    ) -> Result<StateLoad<Self::JournaledAccount<'_>>, DB::Error> {
        let is_cold = self
            .load_account_optional_cold(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        if load_code {
            let account = self.state.get_mut(&address).expect("loaded");
            if account.info.code.is_none() {
                let hash = account.info.code_hash;
                let code = if hash == KECCAK_EMPTY {
                    Bytecode::default()
                } else {
                    self.database.code_by_hash(hash)?
                };
                account.info.code = Some(code);
            }
        }
        let account = self.state.get_mut(&address).expect("loaded");
        Ok(StateLoad::new(
            PevmJournaledAccount::new(
                address,
                account,
                &mut self.journal,
                &mut self.database,
                self.warm_addresses.access_list(),
                self.transaction_id,
                &mut self.pending_write_set,
                &mut self.pending_new_bytecodes,
                self.is_lazy,
                self.from_hash,
                self.to_hash,
                self.tx_value,
                self.is_eip161,
            ),
            is_cold,
        ))
    }

    fn set_code_with_hash(&mut self, address: Address, code: Bytecode, hash: B256) {
        let account = self.state.get_mut(&address).unwrap();
        if !account.is_touched() {
            self.journal
                .push(PevmJournalEntry::AccountTouched { address });
            account.mark_touch();
        }
        self.journal.push(PevmJournalEntry::CodeChange { address });
        account.info.code_hash = hash;
        account.info.code = Some(code);
        // Save what we need before NLL releases the account borrow.
        let balance = account.info.balance;
        let nonce = account.info.nonce;
        let code_to_record = (hash != KECCAK_EMPTY)
            .then(|| account.info.code.clone())
            .flatten();
        // account borrow released by NLL here
        let basic_hash = self.basic_hash_for(address);
        if hash != KECCAK_EMPTY {
            self.pending_write_set.push((
                hash_deterministic(MemoryLocation::CodeHash(address)),
                MemoryValue::CodeHash(hash),
            ));
            if let Some(c) = code_to_record {
                self.pending_new_bytecodes.push((hash, c));
            }
        }
        self.push_basic_ws_with_hash(basic_hash, balance, nonce, hash);
    }

    fn checkpoint(&mut self) -> JournalCheckpoint {
        let cp = JournalCheckpoint {
            log_i: self.logs.len(),
            journal_i: self.journal.len(),
            selfdestructed_i: self.selfdestructed_addresses.len(),
        };
        self.write_set_checkpoints
            .push(self.pending_write_set.len());
        self.depth += 1;
        cp
    }

    fn checkpoint_commit(&mut self) {
        self.write_set_checkpoints.pop();
        self.depth = self.depth.saturating_sub(1);
    }

    fn checkpoint_revert(&mut self, checkpoint: JournalCheckpoint) {
        let is_spurious_dragon = self.cfg.spec.is_enabled_in(SpecId::SPURIOUS_DRAGON);
        self.depth = self.depth.saturating_sub(1);
        if let Some(ws_len) = self.write_set_checkpoints.pop() {
            self.pending_write_set.truncate(ws_len);
        }
        self.logs.truncate(checkpoint.log_i);
        self.selfdestructed_addresses
            .truncate(checkpoint.selfdestructed_i);
        if checkpoint.journal_i < self.journal.len() {
            let state = &mut self.state;
            let ts = &mut self.transient_storage;
            self.journal
                .drain(checkpoint.journal_i..)
                .rev()
                .for_each(|entry| {
                    entry.revert(state, Some(ts), is_spurious_dragon);
                });
        }
    }

    fn create_account_checkpoint(
        &mut self,
        caller: Address,
        address: Address,
        balance: U256,
        spec_id: SpecId,
    ) -> Result<JournalCheckpoint, TransferError> {
        let checkpoint = self.checkpoint();

        let target_acc = self.state.get_mut(&address).unwrap();
        let last_journal = &mut self.journal;

        if target_acc.info.code_hash != KECCAK_EMPTY || target_acc.info.nonce != 0 {
            // End borrows so NLL releases them before checkpoint_revert
            let _ = target_acc;
            let _ = last_journal;
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::CreateCollision);
        }

        let is_created_globally = target_acc.mark_created_locally();
        last_journal.push(PevmJournalEntry::AccountCreated {
            address,
            is_created_globally,
        });
        target_acc.info.code = None;
        if spec_id.is_enabled_in(SpecId::SPURIOUS_DRAGON) {
            target_acc.info.nonce = 1;
        }
        // touch
        if !target_acc.is_touched() {
            last_journal.push(PevmJournalEntry::AccountTouched { address });
            target_acc.mark_touch();
        }
        // Save address account data before the borrow may be released.
        let addr_nonce = target_acc.info.nonce;
        // code_hash is KECCAK_EMPTY: we just set code = None and the collision
        // guard above confirmed code_hash was KECCAK_EMPTY on entry.

        if balance.is_zero() {
            let addr_balance = target_acc.info.balance;
            // NLL releases target_acc/last_journal borrows here
            let addr_hash = self.basic_hash_for(address);
            self.push_basic_ws_with_hash(addr_hash, addr_balance, addr_nonce, KECCAK_EMPTY);
            return Ok(checkpoint);
        }

        let Some(new_balance) = target_acc.info.balance.checked_add(balance) else {
            // End borrows so NLL releases them before checkpoint_revert
            let _ = target_acc;
            let _ = last_journal;
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::OverflowPayment);
        };
        target_acc.info.balance = new_balance;
        // target_acc/last_journal no longer used: NLL releases borrows here

        let caller_account = self.state.get_mut(&caller).unwrap();
        caller_account.info.balance -= balance;
        let caller_balance = caller_account.info.balance;
        let caller_nonce = caller_account.info.nonce;
        let caller_code_hash = caller_account.info.code_hash;
        // caller_account no longer used

        self.journal.push(PevmJournalEntry::BalanceTransfer {
            from: caller,
            to: address,
            balance,
        });
        self.eip7708_transfer_log(caller, address, balance);
        let caller_hash = self.basic_hash_for(caller);
        self.push_basic_ws_with_hash(caller_hash, caller_balance, caller_nonce, caller_code_hash);
        let addr_hash = self.basic_hash_for(address);
        self.push_basic_ws_with_hash(addr_hash, new_balance, addr_nonce, KECCAK_EMPTY);

        Ok(checkpoint)
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn commit_tx(&mut self) {
        self.transient_storage.clear();
        self.depth = 0;
        self.journal.clear();
        self.warm_addresses.clear_coinbase_and_access_list();
        self.transaction_id += 1;
        self.logs.clear();
        self.selfdestructed_addresses.clear();
        // write_set_checkpoints is empty at commit (all sub-call checkpoints were committed)
    }

    fn discard_tx(&mut self) {
        let is_spurious_dragon = self.cfg.spec.is_enabled_in(SpecId::SPURIOUS_DRAGON);
        let state = &mut self.state;
        self.journal.drain(..).rev().for_each(|entry| {
            entry.revert(state, None, is_spurious_dragon);
        });
        self.transient_storage.clear();
        self.depth = 0;
        self.logs.clear();
        self.selfdestructed_addresses.clear();
        self.transaction_id += 1;
        self.warm_addresses.clear_coinbase_and_access_list();
        self.pending_write_set.clear();
        self.write_set_checkpoints.clear();
    }

    fn finalize(&mut self) -> EvmState {
        // Deduplication of the write-set is done in take_write_set(), which must be
        // called before finalize() in the parallel path.  Clear here so the field is
        // always empty at the start of the next transaction (fixes an O(N²) bug in
        // sequential execution where neither clear() nor take_write_set() was called).
        self.pending_write_set.clear();
        self.pending_new_bytecodes.clear();

        // Take & normalise state (mirrors JournalInner::finalize logic).
        self.warm_addresses.clear_coinbase_and_access_list();
        self.selfdestructed_addresses.clear();
        let mut state = std::mem::take(&mut self.state);
        if !self.cfg.spec.is_enabled_in(SpecId::SPURIOUS_DRAGON) {
            for acc in state.values_mut() {
                if acc.is_touched()
                    && acc.is_empty()
                    && !acc.is_selfdestructed()
                    && !acc.is_created()
                {
                    if acc.is_loaded_as_not_existing() {
                        acc.mark_created();
                    } else {
                        acc.unmark_touch();
                    }
                }
            }
        }
        self.logs.clear();
        self.transient_storage.clear();
        self.journal.clear();
        self.depth = 0;
        self.transaction_id = 0;
        state
    }

    fn load_account_info_skip_cold_load(
        &mut self,
        address: Address,
        load_code: bool,
        skip_cold_load: bool,
    ) -> Result<AccountInfoLoad<'_>, JournalLoadError<DB::Error>> {
        let is_cold = self.load_account_optional_cold(address, skip_cold_load)?;
        if load_code {
            let account = self.state.get_mut(&address).expect("loaded");
            if account.info.code.is_none() {
                let hash = account.info.code_hash;
                let code = if hash == KECCAK_EMPTY {
                    Bytecode::default()
                } else {
                    self.database
                        .code_by_hash(hash)
                        .map_err(JournalLoadError::DBError)?
                };
                account.info.code = Some(code);
            }
        }
        let spec = self.cfg.spec;
        let account = self.state.get(&address).expect("loaded");
        let is_empty = account.state_clear_aware_is_empty(spec);
        Ok(AccountInfoLoad::new(&account.info, is_cold, is_empty))
    }
}
