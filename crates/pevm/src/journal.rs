//! Custom journal implementation for PEVM, based on revm's `Journal` but flattened

use core::mem;

use revm::{
    Database,
    context::journal::{JournalCfg, warm_addresses::WarmAddresses},
    context_interface::{
        ErasedError,
        context::{SStoreResult, SelfDestructResult},
        journaled_state::{
            AccountInfoLoad, AccountLoad, JournalCheckpoint, JournalLoadErasedError,
            JournalLoadError, JournalTr, StateLoad, TransferError, account::JournaledAccountTr,
        },
    },
    primitives::{
        Address, AddressSet, B256, HashSet, KECCAK_EMPTY, Log, PRECOMPILE3, StorageKey,
        StorageValue, U256,
        hardfork::SpecId::{self, *},
        map::Entry,
    },
    state::{Account, AccountStatus, Bytecode, EvmStorageSlot, TransientStorage},
};

use crate::{AddressMap, EvmState};

/// Status of selfdestruction revert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum SelfdestructionRevertStatus {
    GloballySelfdestroyed,
    LocallySelfdestroyed,
    RepeatedSelfdestruction,
}

/// Journal entries tracking state changes for checkpoint revert.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum JournalEntry {
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

impl JournalEntry {
    fn revert(
        self,
        state: &mut EvmState,
        transient_storage: &mut TransientStorage,
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
                    SelfdestructionRevertStatus::RepeatedSelfdestruction => (),
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
                let tkey = (address, key);
                if had_value.is_zero() {
                    transient_storage.remove(&tkey);
                } else {
                    transient_storage.insert(tkey, had_value);
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

/// Wraps a mutable account and journal vec so writes can be done with automatic journal recording.
#[derive(Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct JournaledAccount<'a, DB> {
    address: Address,
    account: &'a mut Account,
    journal_entries: &'a mut Vec<JournalEntry>,
    access_list: &'a AddressMap<HashSet<StorageKey>>,
    db: &'a mut DB,
}

#[allow(missing_docs)]
impl<'a, DB: Database> JournaledAccount<'a, DB> {
    #[inline(never)]
    pub fn sload_concrete_error(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&mut EvmStorageSlot>, JournalLoadError<DB::Error>> {
        let is_cold;
        let is_newly_created = self.account.is_created();
        let slot = match self.account.storage.entry(key) {
            Entry::Occupied(occ) => {
                let slot = occ.into_mut();
                // RISE: slot.is_cold is set by sub-call reverts (StorageWarmed revert → mark_cold).
                // transaction_id comparison is always false since state is cleared between txs.
                is_cold = slot.is_cold
                    && self
                        .access_list
                        .get(&self.address)
                        .and_then(|v| v.get(&key))
                        .is_none();
                if is_cold && skip_cold_load {
                    return Err(JournalLoadError::ColdLoadSkipped);
                }
                slot.mark_warm_with_transaction_id(0);
                slot
            }
            Entry::Vacant(vac) => {
                is_cold = self
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
                vac.insert(EvmStorageSlot::new(value, 0))
            }
        };
        if is_cold {
            self.journal_entries.push(JournalEntry::StorageWarmed {
                address: self.address,
                key,
            });
        }
        Ok(StateLoad::new(slot, is_cold))
    }

    #[inline]
    pub fn sstore_concrete_error(
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
            self.journal_entries.push(JournalEntry::StorageChanged {
                address: self.address,
                key,
                had_value: previous_value,
            });
        }
        ret
    }

    #[inline]
    pub fn load_code_preserve_error(&mut self) -> Result<&Bytecode, JournalLoadError<DB::Error>> {
        if self.account.info.code.is_none() {
            let hash = *self.code_hash();
            let code = if hash == KECCAK_EMPTY {
                Bytecode::default()
            } else {
                self.db.code_by_hash(hash)?
            };
            self.account.info.code = Some(code);
        }
        Ok(self.account.info.code.as_ref().unwrap())
    }

    #[inline]
    pub const fn into_account(self) -> &'a Account {
        self.account
    }
}

impl<'a, DB: Database> JournaledAccountTr for JournaledAccount<'a, DB> {
    #[inline]
    fn account(&self) -> &Account {
        self.account
    }
    #[inline]
    fn balance(&self) -> &U256 {
        &self.account.info.balance
    }
    #[inline]
    fn nonce(&self) -> u64 {
        self.account.info.nonce
    }
    #[inline]
    fn code_hash(&self) -> &B256 {
        &self.account.info.code_hash
    }
    #[inline]
    fn code(&self) -> Option<&Bytecode> {
        self.account.info.code.as_ref()
    }
    #[inline]
    fn touch(&mut self) {
        if !self.account.status.is_touched() {
            self.account.mark_touch();
            self.journal_entries.push(JournalEntry::AccountTouched {
                address: self.address,
            });
        }
    }
    #[inline]
    fn unsafe_mark_cold(&mut self) {
        self.account.mark_cold();
    }
    #[inline]
    fn set_balance(&mut self, balance: U256) {
        self.touch();
        if self.account.info.balance != balance {
            self.journal_entries.push(JournalEntry::BalanceChange {
                address: self.address,
                old_balance: self.account.info.balance,
            });
            self.account.info.set_balance(balance);
        }
    }
    #[inline]
    fn incr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(balance) = self.account.info.balance.checked_add(balance) else {
            return false;
        };
        self.set_balance(balance);
        true
    }
    #[inline]
    fn decr_balance(&mut self, balance: U256) -> bool {
        self.touch();
        let Some(balance) = self.account.info.balance.checked_sub(balance) else {
            return false;
        };
        self.set_balance(balance);
        true
    }
    #[inline]
    fn bump_nonce(&mut self) -> bool {
        self.touch();
        let Some(nonce) = self.account.info.nonce.checked_add(1) else {
            return false;
        };
        self.account.info.set_nonce(nonce);
        self.journal_entries.push(JournalEntry::NonceBump {
            address: self.address,
        });
        true
    }
    #[inline]
    fn set_nonce(&mut self, nonce: u64) {
        self.touch();
        let previous_nonce = self.account.info.nonce;
        self.account.info.set_nonce(nonce);
        self.journal_entries.push(JournalEntry::NonceChange {
            address: self.address,
            previous_nonce,
        });
    }
    #[inline]
    fn unsafe_set_nonce(&mut self, nonce: u64) {
        self.account.info.set_nonce(nonce);
    }
    #[inline]
    fn set_code(&mut self, code_hash: B256, code: Bytecode) {
        self.touch();
        self.account.info.set_code_and_hash(code, code_hash);
        self.journal_entries.push(JournalEntry::CodeChange {
            address: self.address,
        });
    }
    #[inline]
    fn set_code_and_hash_slow(&mut self, code: Bytecode) {
        let code_hash = code.hash_slow();
        self.set_code(code_hash, code);
    }
    #[inline]
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
    #[inline]
    fn sload(
        &mut self,
        key: StorageKey,
        skip_cold_load: bool,
    ) -> Result<StateLoad<&mut EvmStorageSlot>, JournalLoadErasedError> {
        self.sload_concrete_error(key, skip_cold_load)
            .map_err(|i| i.map(ErasedError::new))
    }
    #[inline]
    fn sstore(
        &mut self,
        key: StorageKey,
        new: StorageValue,
        skip_cold_load: bool,
    ) -> Result<StateLoad<SStoreResult>, JournalLoadErasedError> {
        self.sstore_concrete_error(key, new, skip_cold_load)
            .map_err(|i| i.map(ErasedError::new))
    }
    #[inline]
    fn load_code(&mut self) -> Result<&Bytecode, JournalLoadErasedError> {
        self.load_code_preserve_error()
            .map_err(|i| i.map(ErasedError::new))
    }
}

/// Forked from revm's `JournalInner` + `Journal` with fields flattened onto one struct.
/// EIP-7708 (Amsterdam) is omitted — neither Ethereum (CANCUN) nor RISE (JOVIAN=Prague) needs it.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct Journal<DB: Database> {
    pub database: DB,
    pub state: EvmState,
    /// EIP-1153 transient storage, cleared after every transaction.
    pub transient_storage: TransientStorage,
    pub logs: Vec<Log>,
    pub depth: usize,
    pub journal: Vec<JournalEntry>,
    pub cfg: JournalCfg,
    pub warm_addresses: WarmAddresses,
}

impl<DB: Database> Journal<DB> {
    pub(crate) fn new(database: DB, cfg: JournalCfg) -> Self {
        Self {
            database,
            state: EvmState::default(),
            transient_storage: TransientStorage::default(),
            logs: Vec::new(),
            depth: 0,
            journal: Vec::new(),
            cfg,
            warm_addresses: WarmAddresses::new(),
        }
    }

    fn extract_state(&mut self) -> EvmState {
        self.warm_addresses.clear_coinbase_and_access_list();

        let mut state = mem::take(&mut self.state);

        if !self.cfg.spec.is_enabled_in(SPURIOUS_DRAGON) {
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

        state
    }

    #[inline]
    fn touch_account(journal: &mut Vec<JournalEntry>, address: Address, account: &mut Account) {
        if !account.is_touched() {
            journal.push(JournalEntry::AccountTouched { address });
            account.mark_touch();
        }
    }

    #[inline]
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
        Ok(load.map(|i| i.into_account()))
    }

    #[inline]
    fn load_account_mut_optional(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<JournaledAccount<'_, DB>>, JournalLoadError<DB::Error>> {
        let mut is_cold = false;
        let account = match self.state.entry(address) {
            Entry::Occupied(entry) => {
                let account = entry.into_mut();
                // RISE: Cold bit is set by sub-call reverts (AccountWarmed revert → mark_cold).
                // transaction_id comparison is always false since state is cleared between txs.
                if account.status.contains(AccountStatus::Cold) {
                    is_cold = self
                        .warm_addresses
                        .check_is_cold(&address, skip_cold_load)?;
                    account.mark_warm_with_transaction_id(0);
                    if account.is_selfdestructed_locally() {
                        account.selfdestruct();
                        account.unmark_selfdestructed_locally();
                    }
                    *account.original_info = account.info.clone();
                    account.unmark_created_locally();
                    self.journal.push(JournalEntry::AccountWarmed { address });
                }
                account
            }
            Entry::Vacant(vac) => {
                is_cold = self
                    .warm_addresses
                    .check_is_cold(&address, skip_cold_load)?;
                let account = self.database.basic(address)?
                    .map(Account::from)
                    .unwrap_or_else(|| Account::new_not_existing(0));
                if is_cold {
                    self.journal.push(JournalEntry::AccountWarmed { address });
                }
                vac.insert(account)
            }
        };

        Ok(StateLoad::new(
            JournaledAccount {
                address,
                account,
                journal_entries: &mut self.journal,
                db: &mut self.database,
                access_list: self.warm_addresses.access_list(),
            },
            is_cold,
        ))
    }

    #[inline]
    fn get_account_mut(&mut self, address: Address) -> Option<JournaledAccount<'_, DB>> {
        let account = self.state.get_mut(&address)?;
        Some(JournaledAccount {
            address,
            account,
            journal_entries: &mut self.journal,
            db: &mut self.database,
            access_list: self.warm_addresses.access_list(),
        })
    }
}

impl<DB: Database> JournalTr for Journal<DB> {
    type Database = DB;
    type State = EvmState;
    type JournaledAccount<'a>
        = JournaledAccount<'a, DB>
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
        mem::take(&mut self.logs)
    }

    fn logs(&self) -> &[Log] {
        &self.logs
    }

    fn log(&mut self, log: Log) {
        self.logs.push(log);
    }

    // finalize() → extract_state() clears all per-tx state; these are no-ops for our usage.
    fn commit_tx(&mut self) {}
    fn discard_tx(&mut self) {}

    fn finalize(&mut self) -> EvmState {
        self.extract_state()
    }

    fn clear(&mut self) {
        // Clear in-place to retain heap allocations for reuse across txs.
        // Pre-Spurious-Dragon fixup in extract_state() is only needed when returning state.
        self.state.clear();
        self.warm_addresses.clear_coinbase_and_access_list();
        self.logs.clear();
        self.transient_storage.clear();
        // self.journal.clear();
        self.depth = 0;
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

    fn touch_account(&mut self, address: Address) {
        if let Some(account) = self.state.get_mut(&address) {
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
            let from_balance = self.state.get_mut(&to).unwrap().info.balance;
            if balance > from_balance {
                return Some(TransferError::OutOfFunds);
            }
            return None;
        }

        if balance.is_zero() {
            Self::touch_account(&mut self.journal, to, self.state.get_mut(&to).unwrap());
            return None;
        }

        let from_account = self.state.get_mut(&from).unwrap();
        Self::touch_account(&mut self.journal, from, from_account);
        let from_balance = &mut from_account.info.balance;
        let Some(from_balance_decr) = from_balance.checked_sub(balance) else {
            return Some(TransferError::OutOfFunds);
        };
        *from_balance = from_balance_decr;

        let to_account = self.state.get_mut(&to).unwrap();
        Self::touch_account(&mut self.journal, to, to_account);
        let to_balance = &mut to_account.info.balance;
        let Some(to_balance_incr) = to_balance.checked_add(balance) else {
            return Some(TransferError::OverflowPayment);
        };
        *to_balance = to_balance_incr;

        self.journal
            .push(JournalEntry::BalanceTransfer { from, to, balance });

        None
    }

    #[allow(deprecated)]
    fn caller_accounting_journal_entry(
        &mut self,
        address: Address,
        old_balance: U256,
        bump_nonce: bool,
    ) {
        self.journal.push(JournalEntry::BalanceChange {
            address,
            old_balance,
        });
        self.journal.push(JournalEntry::AccountTouched { address });
        if bump_nonce {
            self.journal.push(JournalEntry::NonceBump { address });
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
        self.journal.push(JournalEntry::NonceBump { address });
    }

    fn set_code_with_hash(&mut self, address: Address, code: Bytecode, hash: B256) {
        let account = self.state.get_mut(&address).unwrap();
        Self::touch_account(&mut self.journal, address, account);
        self.journal.push(JournalEntry::CodeChange { address });
        account.info.code_hash = hash;
        account.info.code = Some(code);
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
        let is_eip7702_enabled = spec.is_enabled_in(SpecId::PRAGUE);
        let account = self
            .load_account_optional(address, is_eip7702_enabled, false)
            .map_err(JournalLoadError::unwrap_db_error)?;
        let is_empty = account.state_clear_aware_is_empty(spec);

        let mut account_load = StateLoad::new(
            AccountLoad {
                is_delegate_account_cold: None,
                is_empty,
            },
            account.is_cold,
        );

        if let Some(address) = account
            .data
            .info
            .code
            .as_ref()
            .and_then(Bytecode::eip7702_address)
        {
            let delegate_account = self
                .load_account_optional(address, true, false)
                .map_err(JournalLoadError::unwrap_db_error)?;
            account_load.data.is_delegate_account_cold = Some(delegate_account.is_cold);
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
        let checkpoint = JournalCheckpoint {
            log_i: self.logs.len(),
            journal_i: self.journal.len(),
            selfdestructed_i: 0,
        };
        self.depth += 1;
        checkpoint
    }

    fn checkpoint_commit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn checkpoint_revert(&mut self, checkpoint: JournalCheckpoint) {
        let is_spurious_dragon_enabled = self.cfg.spec.is_enabled_in(SPURIOUS_DRAGON);
        let state = &mut self.state;
        let transient_storage = &mut self.transient_storage;
        self.depth = self.depth.saturating_sub(1);
        self.logs.truncate(checkpoint.log_i);
        if checkpoint.journal_i < self.journal.len() {
            self.journal
                .drain(checkpoint.journal_i..)
                .rev()
                .for_each(|entry| {
                    entry.revert(state, transient_storage, is_spurious_dragon_enabled);
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
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::CreateCollision);
        }

        let is_created_globally = target_acc.mark_created_locally();
        last_journal.push(JournalEntry::AccountCreated {
            address,
            is_created_globally,
        });
        target_acc.info.code = None;
        if spec_id.is_enabled_in(SPURIOUS_DRAGON) {
            target_acc.info.nonce = 1;
        }

        Self::touch_account(last_journal, address, target_acc);

        if balance.is_zero() {
            return Ok(checkpoint);
        }

        let Some(new_balance) = target_acc.info.balance.checked_add(balance) else {
            self.checkpoint_revert(checkpoint);
            return Err(TransferError::OverflowPayment);
        };
        target_acc.info.balance = new_balance;

        let caller_account = self.state.get_mut(&caller).unwrap();
        caller_account.info.balance -= balance;

        last_journal.push(JournalEntry::BalanceTransfer {
            from: caller,
            to: address,
            balance,
        });

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
            let acc_balance = self.state.get(&address).unwrap().info.balance;
            let target_account = self.state.get_mut(&target).unwrap();
            Self::touch_account(&mut self.journal, target, target_account);
            target_account.info.balance += acc_balance;
        }

        let acc = self.state.get_mut(&address).unwrap();
        let balance = acc.info.balance;

        let destroyed_status = if !acc.is_selfdestructed() {
            SelfdestructionRevertStatus::GloballySelfdestroyed
        } else if !acc.is_selfdestructed_locally() {
            SelfdestructionRevertStatus::LocallySelfdestroyed
        } else {
            SelfdestructionRevertStatus::RepeatedSelfdestruction
        };

        let is_cancun_enabled = spec.is_enabled_in(CANCUN);

        let journal_entry = if acc.is_created_locally() || !is_cancun_enabled {
            acc.mark_selfdestructed_locally();
            acc.info.balance = U256::ZERO;
            Some(JournalEntry::AccountDestroyed {
                address,
                target,
                destroyed_status,
                had_balance: balance,
            })
        } else if address != target {
            acc.info.balance = U256::ZERO;
            Some(JournalEntry::BalanceTransfer {
                from: address,
                to: target,
                balance,
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
            let previous_value = self
                .transient_storage
                .insert((address, key), value)
                .unwrap_or_default();
            (previous_value != value).then_some(previous_value)
        };

        if let Some(had_value) = had_value {
            self.journal.push(JournalEntry::TransientStorageChange {
                address,
                key,
                had_value,
            });
        }
    }
}
