//! Pevm-native journal (`PevmJournal<DB>`) that records `MvMemory` writes at
//! write time, eliminating post-execution re-hashing and per-account storage
//! allocations from the original extraction loop.

use core::mem;

use smallvec::SmallVec;

use revm::{
    Database,
    context::journal::{JournalCfg, warm_addresses::WarmAddresses},
    context_interface::{
        context::{SStoreResult, SelfDestructResult},
        journaled_state::{
            AccountInfoLoad, AccountLoad, JournalCheckpoint, JournalLoadError, JournalTr,
            StateLoad, TransferError, account::JournaledAccountTr,
            entry::SelfdestructionRevertStatus,
        },
    },
    primitives::{
        Address,
        AddressMap,
        AddressSet,
        B256,
        Bytes,
        HashSet,
        KECCAK_EMPTY,
        Log,
        LogData,
        StorageKey,
        StorageValue,
        U256,
        eip7708::{BURN_LOG_TOPIC, ETH_TRANSFER_LOG_ADDRESS, ETH_TRANSFER_LOG_TOPIC},
        hardfork::SpecId::{self, *},
        // Entry/HashMap from alloy-primitives (hashbrown 0.16) — same version as EvmState,
        // so Entry variants work for both accounts and our custom-hasher maps.
        map::{Entry, HashMap},
    },
    state::{Account, Bytecode, EvmState, EvmStorageSlot, TransientStorage},
};

use crate::{
    AccountBasic, BuildIdentityHasher, MemoryLocation, MemoryLocationHash, MemoryValue, WriteSet,
    hash_deterministic,
};

// WriteSet and dirty both use the workspace hashbrown (0.17). The alloy HashMap
// (0.16) is only used for accounts/storage_slots which need Entry-variant
// compatibility with EvmState.
type DirtyMap = hashbrown::HashMap<MemoryLocationHash, MemoryValue, BuildIdentityHasher>;

/// Undo log entry for `PevmJournal`. Used by `checkpoint_revert` to restore state.
#[derive(Debug)]
pub(crate) enum PevmJournalEntry {
    AccountWarmed(Address),
    AccountTouched(Address),
    BalanceChange {
        address: Address,
        old_balance: U256,
    },
    BalanceTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    NonceBump {
        address: Address,
    },
    NonceChange {
        address: Address,
        previous_nonce: u64,
    },
    CodeChange {
        address: Address,
    },
    AccountCreated {
        address: Address,
        globally: bool,
    },
    AccountDestroyed {
        address: Address,
        target: Address,
        had_balance: U256,
        status: SelfdestructionRevertStatus,
    },
    // 8 bytes vs revm's (address + key) = 52 bytes.
    StorageWarmed {
        hash: MemoryLocationHash,
    },
    StorageChanged {
        address: Address,
        key: StorageKey,
        prev_value: StorageValue,
    },
    TransientStorageChanged {
        address: Address,
        key: StorageKey,
        prev: StorageValue,
    },
}

/// Writes extracted from `PevmJournal` after a transaction.
pub(crate) struct ExtractedWrites {
    pub(crate) write_set: WriteSet,
    pub(crate) new_bytecodes: SmallVec<[(B256, Bytecode); 1]>,
}

/// Pevm-native journal. Implements `JournalTr` with the same semantics as Journal<DB> but
/// records writes in dirty (MvMemory-native form) at write time.
#[derive(Debug)]
pub struct PevmJournal<DB: Database> {
    pub(crate) database: DB,
    /// Account map. Presence = warm; cleared on `set_tx()`.
    pub(crate) accounts: EvmState,
    /// Flat storage cache for all accounts. Presence = warm; cleared on `set_tx()`.
    pub(crate) storage_slots: HashMap<MemoryLocationHash, EvmStorageSlot, BuildIdentityHasher>,
    /// MvMemory-native write buffer. Moved out by `extract()` via `mem::take`.
    pub(crate) dirty: DirtyMap,
    journal: Vec<PevmJournalEntry>,
    new_bytecodes: SmallVec<[(B256, Bytecode); 1]>,
    pub(crate) from_addr: Address,
    pub(crate) from_hash: MemoryLocationHash,
    pub(crate) to_addr: Option<Address>,
    pub(crate) to_hash: Option<MemoryLocationHash>,
    pub(crate) is_lazy: bool,
    pub(crate) logs: Vec<Log>,
    pub(crate) depth: usize,
    pub(crate) cfg: JournalCfg,
    pub(crate) warm_addresses: WarmAddresses,
    pub(crate) transient_storage: TransientStorage,
    pub(crate) selfdestructed_addresses: Vec<Address>,
}

// Compute the dirty write value for a basic (balance/nonce) location.
// Free function to avoid borrow conflicts when holding an account reference.
#[inline]
#[allow(clippy::too_many_arguments)]
fn make_basic_dirty(
    address: Address,
    balance: U256,
    nonce: u64,
    from_addr: Address,
    from_hash: MemoryLocationHash,
    to_addr: Option<Address>,
    to_hash: Option<MemoryLocationHash>,
    is_lazy: bool,
) -> (MemoryLocationHash, MemoryValue) {
    let location = if address == from_addr {
        from_hash
    } else if to_addr == Some(address) {
        to_hash.unwrap()
    } else {
        hash_deterministic(MemoryLocation::Basic(address))
    };
    let value = if is_lazy && location == from_hash {
        MemoryValue::LazySender(U256::MAX - balance)
    } else if is_lazy && Some(location) == to_hash {
        // Use actual balance (net EVM-world addition since VmDb mocked to_addr with None/0).
        MemoryValue::LazyRecipient(balance)
    } else {
        MemoryValue::Basic(AccountBasic { balance, nonce })
    };
    (location, value)
}

// After restoring account.info via revert, recompute dirty for the basic location.
#[inline]
#[allow(clippy::too_many_arguments)]
fn recompute_basic_dirty(
    accounts: &EvmState,
    dirty: &mut DirtyMap,
    address: Address,
    from_addr: Address,
    from_hash: MemoryLocationHash,
    to_addr: Option<Address>,
    to_hash: Option<MemoryLocationHash>,
    is_lazy: bool,
) {
    let Some(account) = accounts.get(&address) else {
        return;
    };
    let location = if address == from_addr {
        from_hash
    } else if to_addr == Some(address) {
        to_hash.unwrap()
    } else {
        hash_deterministic(MemoryLocation::Basic(address))
    };
    let info = &account.info;
    let original = &account.original_info;
    if info.balance == original.balance && info.nonce == original.nonce {
        dirty.remove(&location);
        return;
    }
    let (loc, value) = make_basic_dirty(
        address,
        info.balance,
        info.nonce,
        from_addr,
        from_hash,
        to_addr,
        to_hash,
        is_lazy,
    );
    dirty.insert(loc, value);
}

// After restoring storage_slots[hash].present_value, recompute dirty[hash].
#[inline]
fn recompute_storage_dirty(
    storage_slots: &HashMap<MemoryLocationHash, EvmStorageSlot, BuildIdentityHasher>,
    dirty: &mut DirtyMap,
    hash: MemoryLocationHash,
) {
    if let Some(slot) = storage_slots.get(&hash) {
        if slot.is_changed() {
            dirty.insert(hash, MemoryValue::Storage(slot.present_value));
        } else {
            dirty.remove(&hash);
        }
    }
}

impl<DB: Database> PevmJournal<DB> {
    pub(crate) fn new(database: DB, cfg: JournalCfg) -> Self {
        Self {
            database,
            accounts: EvmState::default(),
            storage_slots: HashMap::with_hasher(BuildIdentityHasher::default()),
            dirty: DirtyMap::with_hasher(BuildIdentityHasher::default()),
            journal: Vec::new(),
            new_bytecodes: SmallVec::new(),
            from_addr: Address::ZERO,
            from_hash: 0,
            to_addr: None,
            to_hash: None,
            is_lazy: false,
            logs: Vec::new(),
            depth: 0,
            cfg,
            warm_addresses: WarmAddresses::new(),
            transient_storage: TransientStorage::default(),
            selfdestructed_addresses: Vec::new(),
        }
    }

    /// Reset per-tx state and set new tx context. Retains allocations.
    pub(crate) fn set_tx(
        &mut self,
        from_addr: Address,
        from_hash: MemoryLocationHash,
        to_addr: Option<Address>,
        to_hash: Option<MemoryLocationHash>,
        is_lazy: bool,
    ) {
        self.from_addr = from_addr;
        self.from_hash = from_hash;
        self.to_addr = to_addr;
        self.to_hash = to_hash;
        self.is_lazy = is_lazy;
        self.accounts.clear();
        self.storage_slots.clear();
        self.dirty.clear();
        self.journal.clear();
        self.new_bytecodes.clear();
    }

    #[inline(never)]
    fn load_account_optional(
        &mut self,
        address: Address,
        load_code: bool,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&Account>, JournalLoadError<DB::Error>> {
        let mut load = self.load_account_mut_optional(address, skip_cold_load)?;
        if load_code {
            load.data.load_code_preserve_error()?;
        }
        Ok(load.map(|acc| acc.into_account()))
    }

    #[inline(never)]
    fn load_account_mut_optional(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<PevmJournalAccount<'_, DB>>, JournalLoadError<DB::Error>> {
        let (account, is_cold) = match self.accounts.entry(address) {
            Entry::Occupied(occ) => (occ.into_mut(), false),
            Entry::Vacant(vac) => {
                let is_cold = self
                    .warm_addresses
                    .check_is_cold(&address, skip_cold_load)?;
                let account = match self.database.basic(address)? {
                    Some(info) => Account::from(info),
                    None => Account::new_not_existing(0),
                };
                if is_cold {
                    self.journal.push(PevmJournalEntry::AccountWarmed(address));
                }
                (vac.insert(account), is_cold)
            }
        };
        Ok(StateLoad::new(
            PevmJournalAccount {
                address,
                account,
                journal: &mut self.journal,
                storage_slots: &mut self.storage_slots,
                dirty: &mut self.dirty,
                db: &mut self.database,
                access_list: self.warm_addresses.access_list(),
                from_addr: self.from_addr,
                from_hash: self.from_hash,
                to_addr: self.to_addr,
                to_hash: self.to_hash,
                is_lazy: self.is_lazy,
            },
            is_cold,
        ))
    }

    #[inline]
    fn get_account_mut(&mut self, address: Address) -> Option<PevmJournalAccount<'_, DB>> {
        let account = self.accounts.get_mut(&address)?;
        Some(PevmJournalAccount {
            address,
            account,
            journal: &mut self.journal,
            storage_slots: &mut self.storage_slots,
            dirty: &mut self.dirty,
            db: &mut self.database,
            access_list: self.warm_addresses.access_list(),
            from_addr: self.from_addr,
            from_hash: self.from_hash,
            to_addr: self.to_addr,
            to_hash: self.to_hash,
            is_lazy: self.is_lazy,
        })
    }

    fn revert_entry(&mut self, entry: PevmJournalEntry) {
        let is_spurious = self.cfg.spec.is_enabled_in(SPURIOUS_DRAGON);
        match entry {
            PevmJournalEntry::AccountWarmed(address) => {
                self.accounts.remove(&address);
            }
            PevmJournalEntry::AccountTouched(address) => {
                if is_spurious && address == revm::primitives::PRECOMPILE3 {
                    return;
                }
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.unmark_touch();
                }
            }
            PevmJournalEntry::BalanceChange {
                address,
                old_balance,
            } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.info.balance = old_balance;
                }
                recompute_basic_dirty(
                    &self.accounts,
                    &mut self.dirty,
                    address,
                    self.from_addr,
                    self.from_hash,
                    self.to_addr,
                    self.to_hash,
                    self.is_lazy,
                );
            }
            PevmJournalEntry::BalanceTransfer { from, to, amount } => {
                if let Some(acc) = self.accounts.get_mut(&from) {
                    acc.info.balance += amount;
                }
                if let Some(acc) = self.accounts.get_mut(&to) {
                    acc.info.balance = acc.info.balance.saturating_sub(amount);
                }
                for addr in [from, to] {
                    recompute_basic_dirty(
                        &self.accounts,
                        &mut self.dirty,
                        addr,
                        self.from_addr,
                        self.from_hash,
                        self.to_addr,
                        self.to_hash,
                        self.is_lazy,
                    );
                }
            }
            PevmJournalEntry::NonceBump { address } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.info.nonce = acc.info.nonce.saturating_sub(1);
                }
                recompute_basic_dirty(
                    &self.accounts,
                    &mut self.dirty,
                    address,
                    self.from_addr,
                    self.from_hash,
                    self.to_addr,
                    self.to_hash,
                    self.is_lazy,
                );
            }
            PevmJournalEntry::NonceChange {
                address,
                previous_nonce,
            } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.info.nonce = previous_nonce;
                }
                recompute_basic_dirty(
                    &self.accounts,
                    &mut self.dirty,
                    address,
                    self.from_addr,
                    self.from_hash,
                    self.to_addr,
                    self.to_hash,
                    self.is_lazy,
                );
            }
            PevmJournalEntry::CodeChange { address } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.info.code_hash = KECCAK_EMPTY;
                    acc.info.code = None;
                }
                let loc = hash_deterministic(MemoryLocation::CodeHash(address));
                self.dirty.remove(&loc);
            }
            PevmJournalEntry::AccountCreated { address, globally } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    acc.unmark_created_locally();
                    if globally {
                        acc.unmark_created();
                    }
                    acc.info.nonce = 0;
                }
                recompute_basic_dirty(
                    &self.accounts,
                    &mut self.dirty,
                    address,
                    self.from_addr,
                    self.from_hash,
                    self.to_addr,
                    self.to_hash,
                    self.is_lazy,
                );
            }
            PevmJournalEntry::AccountDestroyed {
                address,
                target,
                had_balance,
                status,
            } => {
                if let Some(acc) = self.accounts.get_mut(&address) {
                    match status {
                        SelfdestructionRevertStatus::GloballySelfdestroyed => {
                            acc.unmark_selfdestruct();
                            acc.unmark_selfdestructed_locally();
                        }
                        SelfdestructionRevertStatus::LocallySelfdestroyed => {
                            acc.unmark_selfdestructed_locally();
                        }
                        SelfdestructionRevertStatus::RepeatedSelfdestruction => {}
                    }
                    acc.info.balance += had_balance;
                }
                if address != target
                    && let Some(tgt) = self.accounts.get_mut(&target)
                {
                    tgt.info.balance = tgt.info.balance.saturating_sub(had_balance);
                }
                if status == SelfdestructionRevertStatus::GloballySelfdestroyed {
                    self.dirty
                        .remove(&hash_deterministic(MemoryLocation::CodeHash(address)));
                }
                recompute_basic_dirty(
                    &self.accounts,
                    &mut self.dirty,
                    address,
                    self.from_addr,
                    self.from_hash,
                    self.to_addr,
                    self.to_hash,
                    self.is_lazy,
                );
                if address != target {
                    recompute_basic_dirty(
                        &self.accounts,
                        &mut self.dirty,
                        target,
                        self.from_addr,
                        self.from_hash,
                        self.to_addr,
                        self.to_hash,
                        self.is_lazy,
                    );
                }
            }
            PevmJournalEntry::StorageWarmed { hash } => {
                self.storage_slots.remove(&hash);
            }
            PevmJournalEntry::StorageChanged {
                address,
                key,
                prev_value,
            } => {
                let hash = hash_deterministic(MemoryLocation::Storage(address, key));
                if let Some(slot) = self.storage_slots.get_mut(&hash) {
                    slot.present_value = prev_value;
                }
                if let Some(account) = self.accounts.get_mut(&address)
                    && let Some(slot) = account.storage.get_mut(&key)
                {
                    slot.present_value = prev_value;
                }
                recompute_storage_dirty(&self.storage_slots, &mut self.dirty, hash);
            }
            PevmJournalEntry::TransientStorageChanged { address, key, prev } => {
                let tkey = (address, key);
                if prev.is_zero() {
                    self.transient_storage.remove(&tkey);
                } else {
                    self.transient_storage.insert(tkey, prev);
                }
            }
        }
    }

    /// Build the final `EvmState`. Populates account.storage for net-changed slots,
    /// applies pre-Spurious-Dragon normalization, then clears per-tx state.
    pub(crate) fn finalize(&mut self) -> EvmState {
        // account.storage is maintained live during execution (sstore_concrete_error +
        // revert_entry for StorageChanged), so no journal scan is needed here.

        if !self.cfg.spec.is_enabled_in(SPURIOUS_DRAGON) {
            for acc in self.accounts.values_mut() {
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

        self.warm_addresses.clear_coinbase_and_access_list();
        self.selfdestructed_addresses.clear();
        self.logs.clear();
        self.transient_storage.clear();
        self.journal.clear();
        self.storage_slots.clear();
        self.dirty.clear();
        self.new_bytecodes.clear();
        self.depth = 0;

        mem::take(&mut self.accounts)
    }

    /// Extract MvMemory-native writes from dirty. Drains dirty (no re-hashing).
    /// Must be called BEFORE `finalize()` — `finalize()` clears dirty.
    pub(crate) fn extract(&mut self) -> ExtractedWrites {
        // For lazy txs, the recipient must always appear in the write_set so that
        // pevm.rs's post-processing loop can read the recipient's actual storage balance
        // and patch tx_result.state. We use 0 as the addition so that pevm.rs applies
        // a net-zero balance delta — correct for zero-value transfers AND for cases
        // where a non-zero value transfer was subsequently reverted (full revert makes
        // recompute_basic_dirty remove to_hash from dirty; partial revert leaves the
        // correct LazyRecipient(net_balance) from make_basic_dirty).
        if self.is_lazy
            && let Some(to_hash) = self.to_hash
        {
            self.dirty
                .entry(to_hash)
                .or_insert(MemoryValue::LazyRecipient(U256::ZERO));
        }
        ExtractedWrites {
            write_set: mem::take(&mut self.dirty),
            new_bytecodes: mem::take(&mut self.new_bytecodes),
        }
    }

    #[inline]
    fn touch_account(journal: &mut Vec<PevmJournalEntry>, address: Address, account: &mut Account) {
        if !account.is_touched() {
            account.mark_touch();
            journal.push(PevmJournalEntry::AccountTouched(address));
        }
    }

    #[inline]
    fn eip7708_emit_burn_remaining_balance_logs(&mut self) {
        if !self.cfg.spec.is_enabled_in(AMSTERDAM)
            || self.cfg.eip7708_disabled
            || self.cfg.eip7708_delayed_burn_disabled
        {
            return;
        }
        let mut addrs: Vec<(Address, U256)> = self
            .selfdestructed_addresses
            .iter()
            .filter_map(|addr| {
                self.accounts
                    .get(addr)
                    .filter(|a| !a.info.balance.is_zero())
                    .map(|a| (*addr, a.info.balance))
            })
            .collect();
        addrs.sort_unstable_by_key(|(a, _)| *a);
        for (addr, bal) in addrs {
            self.eip7708_burn_log(addr, bal);
        }
    }

    #[inline]
    fn eip7708_transfer_log(&mut self, from: Address, to: Address, balance: U256) {
        if !self.cfg.spec.is_enabled_in(AMSTERDAM) || self.cfg.eip7708_disabled || balance.is_zero()
        {
            return;
        }
        let topics = vec![
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
    fn eip7708_burn_log(&mut self, address: Address, balance: U256) {
        if !self.cfg.spec.is_enabled_in(AMSTERDAM) || self.cfg.eip7708_disabled || balance.is_zero()
        {
            return;
        }
        let topics = vec![BURN_LOG_TOPIC, B256::left_padding_from(address.as_slice())];
        let data = Bytes::copy_from_slice(&balance.to_be_bytes::<32>());
        self.logs.push(Log {
            address: ETH_TRANSFER_LOG_ADDRESS,
            data: LogData::new(topics, data).expect("2 topics is valid"),
        });
    }
}

// ── JournalTr impl for PevmJournal ────────────────────────────────────────────

impl<DB: Database> JournalTr for PevmJournal<DB> {
    type Database = DB;
    type State = EvmState;
    type JournaledAccount<'a>
        = PevmJournalAccount<'a, DB>
    where
        DB: 'a;

    fn new(database: DB) -> Self {
        Self::new(database, JournalCfg::default())
    }

    fn db(&self) -> &DB {
        &self.database
    }

    fn db_mut(&mut self) -> &mut DB {
        &mut self.database
    }

    fn take_logs(&mut self) -> Vec<Log> {
        self.eip7708_emit_burn_remaining_balance_logs();
        mem::take(&mut self.logs)
    }

    fn logs(&self) -> &[Log] {
        &self.logs
    }

    fn log(&mut self, log: Log) {
        self.logs.push(log);
    }

    fn commit_tx(&mut self) {
        self.transient_storage.clear();
        self.depth = 0;
        self.journal.clear();
        self.warm_addresses.clear_coinbase_and_access_list();
        self.logs.clear();
        self.selfdestructed_addresses.clear();
    }

    fn discard_tx(&mut self) {
        while let Some(entry) = self.journal.pop() {
            self.revert_entry(entry);
        }
        self.transient_storage.clear();
        self.depth = 0;
        self.logs.clear();
        self.selfdestructed_addresses.clear();
        self.warm_addresses.clear_coinbase_and_access_list();
    }

    fn finalize(&mut self) -> EvmState {
        self.finalize()
    }

    fn clear(&mut self) {
        self.finalize();
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn set_spec_id(&mut self, spec_id: SpecId) {
        self.cfg.spec = spec_id;
    }

    fn set_eip7708_config(&mut self, disabled: bool, delayed_burn_disabled: bool) {
        self.cfg.eip7708_disabled = disabled;
        self.cfg.eip7708_delayed_burn_disabled = delayed_burn_disabled;
    }

    fn warm_access_list(&mut self, access_list: AddressMap<HashSet<StorageKey>>) {
        self.warm_addresses.set_access_list(access_list);
    }

    fn warm_coinbase_account(&mut self, address: Address) {
        self.warm_addresses.set_coinbase(address);
    }

    fn warm_precompiles(&mut self, addresses: AddressSet) {
        self.warm_addresses.set_precompile_addresses(addresses);
    }

    fn precompile_addresses(&self) -> &AddressSet {
        self.warm_addresses.precompiles()
    }

    fn touch_account(&mut self, address: Address) {
        if let Some(account) = self.accounts.get_mut(&address) {
            Self::touch_account(&mut self.journal, address, account);
        }
    }

    fn transfer(
        &mut self,
        from: Address,
        to: Address,
        balance: U256,
    ) -> Result<Option<TransferError>, DB::Error> {
        self.load_account(from)?;
        self.load_account(to)?;
        Ok(self.transfer_loaded(from, to, balance))
    }

    fn transfer_loaded(
        &mut self,
        from: Address,
        to: Address,
        balance: U256,
    ) -> Option<TransferError> {
        if from == to {
            let from_balance = self.accounts.get(&from).unwrap().info.balance;
            if balance > from_balance {
                return Some(TransferError::OutOfFunds);
            }
            return None;
        }
        if balance.is_zero() {
            let to_acc = self.accounts.get_mut(&to).unwrap();
            Self::touch_account(&mut self.journal, to, to_acc);
            return None;
        }
        {
            let from_acc = self.accounts.get_mut(&from).unwrap();
            Self::touch_account(&mut self.journal, from, from_acc);
            let Some(new_bal) = from_acc.info.balance.checked_sub(balance) else {
                return Some(TransferError::OutOfFunds);
            };
            from_acc.info.balance = new_bal;
        }
        {
            let to_acc = self.accounts.get_mut(&to).unwrap();
            Self::touch_account(&mut self.journal, to, to_acc);
            let Some(new_bal) = to_acc.info.balance.checked_add(balance) else {
                return Some(TransferError::OverflowPayment);
            };
            to_acc.info.balance = new_bal;
        }
        self.journal.push(PevmJournalEntry::BalanceTransfer {
            from,
            to,
            amount: balance,
        });
        self.eip7708_transfer_log(from, to, balance);
        // Update dirty for both accounts.
        for addr in [from, to] {
            let info = self.accounts.get(&addr).unwrap().info.clone();
            let (loc, val) = make_basic_dirty(
                addr,
                info.balance,
                info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }
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
        self.journal.push(PevmJournalEntry::AccountTouched(address));
        if bump_nonce {
            self.journal.push(PevmJournalEntry::NonceBump { address });
        }
        // Framework already modified the account; update dirty with current state.
        if let Some(acc) = self.accounts.get(&address) {
            let (loc, val) = make_basic_dirty(
                address,
                acc.info.balance,
                acc.info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }
    }

    fn balance_incr(&mut self, address: Address, balance: U256) -> Result<(), DB::Error> {
        let mut account = self
            .load_account_mut_optional(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?
            .data;
        account.incr_balance(balance);
        Ok(())
    }

    #[allow(deprecated)]
    fn nonce_bump_journal_entry(&mut self, address: Address) {
        self.journal.push(PevmJournalEntry::NonceBump { address });
        if let Some(acc) = self.accounts.get(&address) {
            let (loc, val) = make_basic_dirty(
                address,
                acc.info.balance,
                acc.info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }
    }

    fn set_code_with_hash(&mut self, address: Address, code: Bytecode, hash: B256) {
        let account = self.accounts.get_mut(&address).unwrap();
        Self::touch_account(&mut self.journal, address, account);
        account.info.code_hash = hash;
        account.info.code = Some(code.clone());
        self.journal.push(PevmJournalEntry::CodeChange { address });
        if hash != KECCAK_EMPTY {
            let loc = hash_deterministic(MemoryLocation::CodeHash(address));
            self.dirty.insert(loc, MemoryValue::CodeHash(hash));
            self.new_bytecodes.push((hash, code));
        }
    }

    fn load_account(&mut self, address: Address) -> Result<StateLoad<&Account>, DB::Error> {
        self.load_account_optional(address, false, false)
            .map_err(JournalLoadError::unwrap_db_error)
    }

    fn load_account_with_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&Account>, DB::Error> {
        self.load_account_optional(address, true, false)
            .map_err(JournalLoadError::unwrap_db_error)
    }

    fn load_account_delegated(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<AccountLoad>, DB::Error> {
        let spec = self.cfg.spec;
        let is_eip7702 = spec.is_enabled_in(SpecId::PRAGUE);
        let account = self
            .load_account_optional(address, is_eip7702, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        let is_empty = account.state_clear_aware_is_empty(spec);
        let mut account_load = StateLoad::new(
            AccountLoad {
                is_delegate_account_cold: None,
                is_empty,
            },
            account.is_cold,
        );
        if let Some(delegate_addr) = account
            .data
            .info
            .code
            .as_ref()
            .and_then(Bytecode::eip7702_address)
        {
            let delegate = self
                .load_account_optional(delegate_addr, true, false)
                .map_err(JournalLoadError::unwrap_db_error)?;
            account_load.data.is_delegate_account_cold = Some(delegate.is_cold);
        }
        Ok(account_load)
    }

    fn load_account_mut_skip_cold_load(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<Self::JournaledAccount<'_>>, JournalLoadError<DB::Error>> {
        self.load_account_mut_optional(address, skip_cold_load)
    }

    fn load_account_mut_optional_code(
        &mut self,
        address: Address,
        load_code: bool,
    ) -> Result<StateLoad<Self::JournaledAccount<'_>>, DB::Error> {
        let mut load = self
            .load_account_mut_optional(address, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        if load_code {
            load.data
                .load_code_preserve_error()
                .map_err(JournalLoadError::unwrap_db_error)?;
        }
        Ok(load)
    }

    fn load_account_info_skip_cold_load(
        &mut self,
        address: Address,
        load_code: bool,
        skip_cold_load: bool,
    ) -> Result<AccountInfoLoad<'_>, JournalLoadError<DB::Error>> {
        let spec = self.cfg.spec;
        self.load_account_optional(address, load_code, skip_cold_load)
            .map(|a| {
                AccountInfoLoad::new(&a.data.info, a.is_cold, a.state_clear_aware_is_empty(spec))
            })
    }

    fn checkpoint(&mut self) -> JournalCheckpoint {
        let cp = JournalCheckpoint {
            log_i: self.logs.len(),
            journal_i: self.journal.len(),
            selfdestructed_i: self.selfdestructed_addresses.len(),
        };
        self.depth += 1;
        cp
    }

    fn checkpoint_commit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn checkpoint_revert(&mut self, checkpoint: JournalCheckpoint) {
        self.depth = self.depth.saturating_sub(1);
        self.logs.truncate(checkpoint.log_i);
        self.selfdestructed_addresses
            .truncate(checkpoint.selfdestructed_i);
        while self.journal.len() > checkpoint.journal_i {
            let entry = self.journal.pop().unwrap();
            self.revert_entry(entry);
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

        let target_acc = self.accounts.get_mut(&address).unwrap();
        if target_acc.info.code_hash != KECCAK_EMPTY || target_acc.info.nonce != 0 {
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::CreateCollision);
        }

        let is_globally = target_acc.mark_created_locally();
        self.journal.push(PevmJournalEntry::AccountCreated {
            address,
            globally: is_globally,
        });
        target_acc.info.code = None;
        if spec_id.is_enabled_in(SPURIOUS_DRAGON) {
            target_acc.info.nonce = 1;
        }
        Self::touch_account(&mut self.journal, address, target_acc);

        // Always write dirty for the new account so nonce=1 is visible in MvMemory.
        {
            let info = self.accounts.get(&address).unwrap().info.clone();
            let (loc, val) = make_basic_dirty(
                address,
                info.balance,
                info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }

        if balance.is_zero() {
            return Ok(checkpoint);
        }

        let Some(new_target_bal) = self
            .accounts
            .get(&address)
            .unwrap()
            .info
            .balance
            .checked_add(balance)
        else {
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::OverflowPayment);
        };
        self.accounts.get_mut(&address).unwrap().info.balance = new_target_bal;
        self.accounts.get_mut(&caller).unwrap().info.balance -= balance;

        self.journal.push(PevmJournalEntry::BalanceTransfer {
            from: caller,
            to: address,
            amount: balance,
        });
        self.eip7708_transfer_log(caller, address, balance);

        for addr in [caller, address] {
            let info = self.accounts.get(&addr).unwrap().info.clone();
            let (loc, val) = make_basic_dirty(
                addr,
                info.balance,
                info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }

        Ok(checkpoint)
    }

    fn selfdestruct(
        &mut self,
        address: Address,
        target: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SelfDestructResult>, JournalLoadError<DB::Error>> {
        let spec = self.cfg.spec;
        let account_load = self.load_account_optional(target, false, skip_cold_load)?;
        let is_cold = account_load.is_cold;
        let is_empty = account_load.state_clear_aware_is_empty(spec);

        if address != target {
            let acc_balance = self.accounts.get(&address).unwrap().info.balance;
            let target_acc = self.accounts.get_mut(&target).unwrap();
            Self::touch_account(&mut self.journal, target, target_acc);
            target_acc.info.balance += acc_balance;
            let info = target_acc.info.clone();
            let (loc, val) = make_basic_dirty(
                target,
                info.balance,
                info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
        }

        let acc = self.accounts.get_mut(&address).unwrap();
        let balance = acc.info.balance;
        let destroyed_status = if !acc.is_selfdestructed() {
            SelfdestructionRevertStatus::GloballySelfdestroyed
        } else if !acc.is_selfdestructed_locally() {
            SelfdestructionRevertStatus::LocallySelfdestroyed
        } else {
            SelfdestructionRevertStatus::RepeatedSelfdestruction
        };

        let is_cancun = spec.is_enabled_in(CANCUN);
        let journal_entry = if acc.is_created_locally() || !is_cancun {
            if destroyed_status == SelfdestructionRevertStatus::GloballySelfdestroyed
                && !self.cfg.eip7708_delayed_burn_disabled
            {
                self.selfdestructed_addresses.push(address);
            }
            acc.mark_selfdestructed_locally();
            acc.info.balance = U256::ZERO;
            // CodeHash → SelfDestructed triggers sequential fallback for later reads.
            // Basic is intentionally NOT written — matches old vm.rs extraction loop.
            self.dirty.insert(
                hash_deterministic(MemoryLocation::CodeHash(address)),
                MemoryValue::SelfDestructed,
            );
            if target == address {
                self.eip7708_burn_log(address, balance);
            } else {
                self.eip7708_transfer_log(address, target, balance);
            }
            Some(PevmJournalEntry::AccountDestroyed {
                address,
                target,
                had_balance: balance,
                status: destroyed_status,
            })
        } else if address != target {
            // Post-Cancun, not created locally: balance-only transfer, code not wiped.
            acc.info.balance = U256::ZERO;
            let (loc, val) = make_basic_dirty(
                address,
                U256::ZERO,
                acc.info.nonce,
                self.from_addr,
                self.from_hash,
                self.to_addr,
                self.to_hash,
                self.is_lazy,
            );
            self.dirty.insert(loc, val);
            self.eip7708_transfer_log(address, target, balance);
            Some(PevmJournalEntry::BalanceTransfer {
                from: address,
                to: target,
                amount: balance,
            })
        } else {
            None
        };

        if let Some(entry) = journal_entry {
            self.journal.push(entry);
        }

        Ok(StateLoad {
            data: SelfDestructResult {
                had_value: !balance.is_zero(),
                target_exists: !is_empty,
                previously_destroyed: destroyed_status
                    == SelfdestructionRevertStatus::RepeatedSelfdestruction,
            },
            is_cold,
        })
    }

    fn sload_skip_cold_load(
        &mut self,
        address: Address,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<StorageValue>, JournalLoadError<DB::Error>> {
        let Some(mut account) = self.get_account_mut(address) else {
            return Err(JournalLoadError::ColdLoadSkipped);
        };
        account
            .sload_concrete_error(key, skip_cold_load)
            .map(|s| s.map(|s| s.present_value))
    }

    fn sstore_skip_cold_load(
        &mut self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadError<DB::Error>> {
        let Some(mut account) = self.get_account_mut(address) else {
            return Err(JournalLoadError::ColdLoadSkipped);
        };
        account.sstore_concrete_error(key, value, skip_cold_load)
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
        if let Some(prev) = had_value {
            self.journal
                .push(PevmJournalEntry::TransientStorageChanged { address, key, prev });
        }
    }
}

// ── PevmJournalAccount ────────────────────────────────────────────────────────

/// Borrowed view into a single account within [`PevmJournal`], satisfying [`JournaledAccountTr`].
#[allow(missing_debug_implementations)]
pub struct PevmJournalAccount<'a, DB: Database> {
    pub(crate) address: Address,
    pub(crate) account: &'a mut Account,
    journal: &'a mut Vec<PevmJournalEntry>,
    storage_slots: &'a mut HashMap<MemoryLocationHash, EvmStorageSlot, BuildIdentityHasher>,
    dirty: &'a mut DirtyMap,
    db: &'a mut DB,
    access_list: &'a AddressMap<HashSet<StorageKey>>,
    from_addr: Address,
    from_hash: MemoryLocationHash,
    to_addr: Option<Address>,
    to_hash: Option<MemoryLocationHash>,
    is_lazy: bool,
}

impl<'a, DB: Database> PevmJournalAccount<'a, DB> {
    pub(crate) fn sload_concrete_error(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&mut EvmStorageSlot>, JournalLoadError<DB::Error>> {
        let hash = hash_deterministic(MemoryLocation::Storage(self.address, key));
        match self.storage_slots.entry(hash) {
            Entry::Occupied(occ) => Ok(StateLoad::new(occ.into_mut(), false)),
            Entry::Vacant(vac) => {
                let is_cold = self
                    .access_list
                    .get(&self.address)
                    .and_then(|s| s.get(&key))
                    .is_none();
                if is_cold && skip_cold_load {
                    return Err(JournalLoadError::ColdLoadSkipped);
                }
                let value = if self.account.is_created() {
                    StorageValue::ZERO
                } else {
                    self.db.storage(self.address, key)?
                };
                let slot = vac.insert(EvmStorageSlot::new(value, 0));
                if is_cold {
                    self.journal.push(PevmJournalEntry::StorageWarmed { hash });
                }
                Ok(StateLoad::new(slot, is_cold))
            }
        }
    }

    pub(crate) fn sstore_concrete_error(
        &mut self,
        key: StorageKey,
        new: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadError<DB::Error>> {
        self.touch();
        let StateLoad {
            data: slot,
            is_cold,
        } = self.sload_concrete_error(key, skip_cold_load)?;
        let original_value = slot.original_value();
        let present_value = slot.present_value();
        let result = Ok(StateLoad::new(
            SStoreResult {
                original_value,
                present_value,
                new_value: new,
            },
            is_cold,
        ));
        if present_value != new {
            slot.present_value = new; // last use of slot — borrow of storage_slots ends here
            // Keep account.storage in sync so finalize()'s EvmState reflects net-changed slots.
            // or_insert initializes with correct original_value on first write.
            self.account
                .storage
                .entry(key)
                .or_insert_with(|| EvmStorageSlot::new(original_value, 0))
                .present_value = new;
            self.journal.push(PevmJournalEntry::StorageChanged {
                address: self.address,
                key,
                prev_value: present_value,
            });
            let hash = hash_deterministic(MemoryLocation::Storage(self.address, key));
            if new == original_value {
                self.dirty.remove(&hash);
            } else {
                self.dirty.insert(hash, MemoryValue::Storage(new));
            }
        }
        result
    }

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

    pub(crate) const fn into_account(self) -> &'a Account {
        self.account
    }

    #[inline]
    fn make_dirty(&self, balance: U256, nonce: u64) -> (MemoryLocationHash, MemoryValue) {
        make_basic_dirty(
            self.address,
            balance,
            nonce,
            self.from_addr,
            self.from_hash,
            self.to_addr,
            self.to_hash,
            self.is_lazy,
        )
    }
}

impl<'a, DB: Database> JournaledAccountTr for PevmJournalAccount<'a, DB> {
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

    fn touch(&mut self) {
        if !self.account.is_touched() {
            self.account.mark_touch();
            self.journal
                .push(PevmJournalEntry::AccountTouched(self.address));
        }
    }

    fn unsafe_mark_cold(&mut self) {
        self.account.mark_cold();
    }

    fn set_balance(&mut self, balance: U256) {
        self.touch();
        if self.account.info.balance != balance {
            let old = self.account.info.balance;
            self.journal.push(PevmJournalEntry::BalanceChange {
                address: self.address,
                old_balance: old,
            });
            self.account.info.balance = balance;
            let (loc, val) = self.make_dirty(balance, self.account.info.nonce);
            self.dirty.insert(loc, val);
        }
    }

    fn incr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(new) = self.account.info.balance.checked_add(balance) else {
            return false;
        };
        self.set_balance(new);
        true
    }

    fn decr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(new) = self.account.info.balance.checked_sub(balance) else {
            return false;
        };
        self.set_balance(new);
        true
    }

    fn bump_nonce(&mut self) -> bool {
        self.touch();
        let Some(nonce) = self.account.info.nonce.checked_add(1) else {
            return false;
        };
        self.account.info.nonce = nonce;
        self.journal.push(PevmJournalEntry::NonceBump {
            address: self.address,
        });
        let (loc, val) = self.make_dirty(self.account.info.balance, nonce);
        self.dirty.insert(loc, val);
        true
    }

    fn set_nonce(&mut self, nonce: u64) {
        self.touch();
        let prev = self.account.info.nonce;
        self.account.info.nonce = nonce;
        self.journal.push(PevmJournalEntry::NonceChange {
            address: self.address,
            previous_nonce: prev,
        });
        let (loc, val) = self.make_dirty(self.account.info.balance, nonce);
        self.dirty.insert(loc, val);
    }

    fn unsafe_set_nonce(&mut self, nonce: u64) {
        self.account.info.nonce = nonce;
    }

    fn set_code(&mut self, code_hash: B256, code: Bytecode) {
        self.touch();
        self.account.info.code_hash = code_hash;
        self.account.info.code = Some(code);
        self.journal.push(PevmJournalEntry::CodeChange {
            address: self.address,
        });
        if code_hash != KECCAK_EMPTY {
            let loc = hash_deterministic(MemoryLocation::CodeHash(self.address));
            self.dirty.insert(loc, MemoryValue::CodeHash(code_hash));
        }
    }

    fn set_code_and_hash_slow(&mut self, code: Bytecode) {
        let hash = code.hash_slow();
        self.set_code(hash, code);
    }

    fn delegate(&mut self, address: Address) {
        let (bytecode, hash) = if address.is_zero() {
            (Bytecode::default(), KECCAK_EMPTY)
        } else {
            let bc = Bytecode::new_eip7702(address);
            let h = bc.hash_slow();
            (bc, h)
        };
        self.touch();
        self.set_code(hash, bytecode);
        self.bump_nonce();
    }

    fn sload(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<
        StateLoad<&mut EvmStorageSlot>,
        revm::context_interface::journaled_state::JournalLoadErasedError,
    > {
        use revm::context_interface::ErasedError;
        self.sload_concrete_error(key, skip_cold_load)
            .map_err(|e| e.map(ErasedError::new))
    }

    fn sstore(
        &mut self,
        key: StorageKey,
        new: StorageValue,
        skip_cold_load: bool,
    ) -> Result<
        StateLoad<SStoreResult>,
        revm::context_interface::journaled_state::JournalLoadErasedError,
    > {
        use revm::context_interface::ErasedError;
        self.sstore_concrete_error(key, new, skip_cold_load)
            .map_err(|e| e.map(ErasedError::new))
    }

    fn load_code(
        &mut self,
    ) -> Result<&Bytecode, revm::context_interface::journaled_state::JournalLoadErasedError> {
        use revm::context_interface::ErasedError;
        self.load_code_preserve_error()
            .map_err(|e| e.map(ErasedError::new))
    }
}
