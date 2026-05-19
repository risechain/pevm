//! Custom journal implementation for PEVM, based on revm's `Journal` but flattened

use core::mem;

use revm::{
    Database,
    context::journal::{JournalCfg, JournalEntry, JournalEntryTr, warm_addresses::WarmAddresses},
    context_interface::{
        context::{SStoreResult, SelfDestructResult},
        journaled_state::{
            AccountInfoLoad, AccountLoad, JournalCheckpoint, JournalLoadError, JournalTr,
            StateLoad, TransferError,
            account::{JournaledAccount, JournaledAccountTr},
            entry::SelfdestructionRevertStatus,
        },
    },
    primitives::{
        Address, AddressMap, AddressSet, B256, Bytes, HashSet, KECCAK_EMPTY, Log, LogData,
        StorageKey, StorageValue, U256,
        eip7708::{BURN_LOG_TOPIC, ETH_TRANSFER_LOG_ADDRESS, ETH_TRANSFER_LOG_TOPIC},
        hardfork::SpecId::{self, *},
        hints_util::unlikely,
        map::Entry,
    },
    state::{Account, Bytecode, EvmState, TransientStorage},
};

/// All fields from revm's `JournalInner` flattened directly onto this struct,
/// alongside `database`. Implements `JournalTr` with identical behavior to
/// `revm::context::Journal<DB>` — no extra wrapping layer.
#[derive(Debug)]
pub struct Journal<DB: Database> {
    /// Database for state access.
    pub database: DB,
    /// The current state.
    pub state: EvmState,
    /// Transient storage (EIP-1153), discarded after every transaction.
    pub transient_storage: TransientStorage,
    /// Emitted logs.
    pub logs: Vec<Log>,
    /// Current call depth.
    pub depth: usize,
    /// Journal of state changes for checkpoint-based revert.
    pub journal: Vec<JournalEntry>,
    /// Number of transactions executed (including reverted).
    pub transaction_id: usize,
    /// Spec ID and EIP-7708 flags.
    pub cfg: JournalCfg,
    /// Warm address tracking (coinbase, precompiles, access list).
    pub warm_addresses: WarmAddresses,
    /// Addresses self-destructed for the first time in this transaction (EIP-7708).
    pub selfdestructed_addresses: Vec<Address>,
}

// ── Helpers called from multiple places ──────────────────────────────────────
//
// These inherent methods are kept because they are called from more than one
// `JournalTr` method (or, in the case of `new`/`finalize`, from outside the
// trait impl as well).  Everything that has exactly one caller has been inlined
// directly into that caller inside the `JournalTr` impl below.
impl<DB: Database> Journal<DB> {
    pub(crate) fn new(database: DB, cfg: JournalCfg) -> Self {
        Self {
            database,
            state: EvmState::default(),
            transient_storage: TransientStorage::default(),
            logs: Vec::new(),
            depth: 0,
            journal: Vec::new(),
            transaction_id: 0,
            cfg,
            warm_addresses: WarmAddresses::new(),
            selfdestructed_addresses: Vec::new(),
        }
    }

    fn finalize(&mut self) -> EvmState {
        self.warm_addresses.clear_coinbase_and_access_list();
        self.selfdestructed_addresses.clear();

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
        self.transaction_id = 0;

        state
    }

    #[inline]
    fn eip7708_emit_burn_remaining_balance_logs(&mut self) {
        if !self.cfg.spec.is_enabled_in(AMSTERDAM)
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

        addresses_with_balance.sort_unstable_by_key(|(addr, _)| *addr);

        for (address, balance) in addresses_with_balance {
            self.eip7708_burn_log(address, balance);
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

    /// Append an EIP-7708 burn log.  Called from `JournalTr::selfdestruct` and
    /// `eip7708_emit_burn_remaining_balance_logs`.
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

    /// Touch `account` at `address`, recording a journal entry only on the
    /// first touch.  Called from `JournalTr::touch_account`,
    /// `JournalTr::transfer_loaded`, `JournalTr::selfdestruct`,
    /// `JournalTr::set_code_with_hash`, and
    /// `JournalTr::create_account_checkpoint`.
    #[inline]
    fn touch_account(journal: &mut Vec<JournalEntry>, address: Address, account: &mut Account) {
        if !account.is_touched() {
            journal.push(JournalEntry::account_touched(address));
            account.mark_touch();
        }
    }

    /// Load an account (optionally with code), optionally skipping the cold
    /// access penalty.  Returns a shared reference inside a `StateLoad`.
    /// Called from `JournalTr::load_account`, `JournalTr::load_account_with_code`,
    /// `JournalTr::load_account_delegated`, `JournalTr::selfdestruct`,
    /// `JournalTr::load_account_mut_optional_code`, and
    /// `JournalTr::load_account_info_skip_cold_load`.
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
        Ok(load.map(|i| i.into_account()))
    }

    /// Load a mutable journaled account, optionally skipping the cold access
    /// penalty.  Called from `load_account_optional`,
    /// `JournalTr::load_account_mut_skip_cold_load`,
    /// `JournalTr::load_account_mut_optional_code`, and
    /// `JournalTr::balance_incr`.
    #[inline(never)]
    fn load_account_mut_optional(
        &mut self,
        address: Address,
        skip_cold_load: bool,
    ) -> Result<StateLoad<JournaledAccount<'_, DB, JournalEntry>>, JournalLoadError<DB::Error>>
    {
        let (account, is_cold) = match self.state.entry(address) {
            Entry::Occupied(entry) => {
                let account = entry.into_mut();
                let mut is_cold = account.is_cold_transaction_id(self.transaction_id);

                if unlikely(is_cold) {
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
                    self.journal.push(JournalEntry::account_warmed(address));
                }
                (account, is_cold)
            }
            Entry::Vacant(vac) => {
                let is_cold = self
                    .warm_addresses
                    .check_is_cold(&address, skip_cold_load)?;

                let account = if let Some(account) = self.database.basic(address)? {
                    let mut account: Account = account.into();
                    account.transaction_id = self.transaction_id;
                    account
                } else {
                    Account::new_not_existing(self.transaction_id)
                };

                if is_cold {
                    self.journal.push(JournalEntry::account_warmed(address));
                }

                (vac.insert(account), is_cold)
            }
        };

        Ok(StateLoad::new(
            JournaledAccount::new(
                address,
                account,
                &mut self.journal,
                &mut self.database,
                self.warm_addresses.access_list(),
                self.transaction_id,
            ),
            is_cold,
        ))
    }

    #[inline]
    fn get_account_mut(
        &mut self,
        address: Address,
    ) -> Option<JournaledAccount<'_, DB, JournalEntry>> {
        let account = self.state.get_mut(&address)?;
        Some(JournaledAccount::new(
            address,
            account,
            &mut self.journal,
            &mut self.database,
            self.warm_addresses.access_list(),
            self.transaction_id,
        ))
    }
}

// ── JournalTr implementation ─────────────────────────────────────────────────

impl<DB: Database> JournalTr for Journal<DB> {
    type Database = DB;
    type State = EvmState;
    type JournaledAccount<'a>
        = JournaledAccount<'a, DB, JournalEntry>
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
        self.transaction_id += 1;
        self.logs.clear();
        self.selfdestructed_addresses.clear();
    }

    fn discard_tx(&mut self) {
        let is_spurious_dragon_enabled = self.cfg.spec.is_enabled_in(SPURIOUS_DRAGON);
        self.journal.drain(..).rev().for_each(|entry| {
            entry.revert(&mut self.state, None, is_spurious_dragon_enabled);
        });
        self.transient_storage.clear();
        self.depth = 0;
        self.logs.clear();
        self.selfdestructed_addresses.clear();
        self.transaction_id += 1;
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
            .push(JournalEntry::balance_transfer(from, to, balance));
        self.eip7708_transfer_log(from, to, balance);

        None
    }

    #[allow(deprecated)]
    fn caller_accounting_journal_entry(
        &mut self,
        address: Address,
        old_balance: U256,
        bump_nonce: bool,
    ) {
        self.journal
            .push(JournalEntry::balance_changed(address, old_balance));
        self.journal.push(JournalEntry::account_touched(address));
        if bump_nonce {
            self.journal.push(JournalEntry::nonce_bumped(address));
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
        self.journal.push(JournalEntry::nonce_bumped(address));
    }

    fn set_code_with_hash(&mut self, address: Address, code: Bytecode, hash: B256) {
        let account = self.state.get_mut(&address).unwrap();
        Self::touch_account(&mut self.journal, address, account);
        self.journal.push(JournalEntry::code_changed(address));
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
            selfdestructed_i: self.selfdestructed_addresses.len(),
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
        self.selfdestructed_addresses
            .truncate(checkpoint.selfdestructed_i);
        if checkpoint.journal_i < self.journal.len() {
            self.journal
                .drain(checkpoint.journal_i..)
                .rev()
                .for_each(|entry| {
                    entry.revert(state, Some(transient_storage), is_spurious_dragon_enabled);
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
        last_journal.push(JournalEntry::account_created(address, is_created_globally));
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

        last_journal.push(JournalEntry::balance_transfer(caller, address, balance));
        self.eip7708_transfer_log(caller, address, balance);

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
            if destroyed_status == SelfdestructionRevertStatus::GloballySelfdestroyed
                && !self.cfg.eip7708_delayed_burn_disabled
            {
                self.selfdestructed_addresses.push(address);
            }

            acc.mark_selfdestructed_locally();
            acc.info.balance = U256::ZERO;

            if target == address {
                self.eip7708_burn_log(address, balance);
            } else {
                self.eip7708_transfer_log(address, target, balance);
            }
            Some(JournalEntry::account_destroyed(
                address,
                target,
                destroyed_status,
                balance,
            ))
        } else if address != target {
            acc.info.balance = U256::ZERO;
            self.eip7708_transfer_log(address, target, balance);
            Some(JournalEntry::balance_transfer(address, target, balance))
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
            self.journal.push(JournalEntry::transient_storage_changed(
                address, key, had_value,
            ));
        }
    }
}
