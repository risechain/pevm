// A storage that stores data in memory.

use std::fmt::Debug;

use ahash::AHashMap;
use alloy_primitives::{keccak256, Address, Bytes, B256, U256};
use revm::db::PlainAccount;

use crate::{AccountBasic, Storage};

/// An account stored in memory.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InMemoryAccount {
    /// The account's basic information.
    pub basic: AccountBasic,
    /// The account's storage.
    pub storage: AHashMap<U256, U256>,
}

impl From<PlainAccount> for InMemoryAccount {
    fn from(account: PlainAccount) -> Self {
        InMemoryAccount {
            basic: account.info.into(),
            storage: account.storage.into_iter().collect(),
        }
    }
}

impl InMemoryAccount {
    /// Checks if the account is empty.
    /// An account is considered empty if its code is empty, balance is zero, and nonce is zero.
    pub fn is_empty(&self) -> bool {
        self.basic.code.is_empty() && self.basic.balance == U256::ZERO && self.basic.nonce == 0
    }

    /// Converts a `revm::primitives::Account` into an `Option<InMemoryAccount>`.
    /// Returns `Some(InMemoryAccount)` if the account is not self-destructed, otherwise returns `None`.
    pub fn from_revm_account(account: revm::primitives::Account) -> Option<Self> {
        assert!(account.is_touched());
        if account.is_selfdestructed() {
            None
        } else {
            Some(InMemoryAccount {
                basic: account.info.into(),
                storage: account
                    .storage
                    .iter()
                    .map(|(k, v)| (*k, v.present_value))
                    .collect(),
            })
        }
    }
}

/// Fetch state data via RPC to execute.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    accounts: AHashMap<Address, InMemoryAccount>,
    block_hashes: AHashMap<U256, B256>,
}

impl InMemoryStorage {
    /// Create a new InMemoryStorage
    pub fn new(
        accounts: impl IntoIterator<Item = (Address, impl Into<InMemoryAccount>)>,
        block_hashes: impl IntoIterator<Item = (U256, B256)>,
    ) -> Self {
        InMemoryStorage {
            accounts: accounts
                .into_iter()
                .map(|(addr, acc)| (addr, acc.into()))
                .collect(),
            block_hashes: block_hashes.into_iter().collect(),
        }
    }

    /// Insert an account
    pub fn insert_account(&mut self, address: Address, account: InMemoryAccount) {
        self.accounts.insert(address, account);
    }
}

impl Storage for InMemoryStorage {
    // TODO: More proper error handling
    type Error = ();

    fn basic(&self, address: Address) -> Result<Option<AccountBasic>, Self::Error> {
        Ok(self
            .accounts
            .get(&address)
            .map(|account| account.basic.clone()))
    }

    fn code_by_hash(&self, code_hash: B256) -> Result<Bytes, Self::Error> {
        for account in self.accounts.values() {
            if account.basic.code_hash == Some(code_hash) {
                return Ok(account.basic.code.clone());
            }
        }
        Ok(Bytes::default())
    }

    fn has_storage(&self, address: Address) -> Result<bool, Self::Error> {
        Ok(self
            .accounts
            .get(&address)
            .is_some_and(|account| !account.storage.is_empty()))
    }

    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self
            .accounts
            .get(&address)
            .and_then(|account| account.storage.get(&index))
            .cloned()
            .unwrap_or_default())
    }

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        Ok(self
            .block_hashes
            .get(&number)
            .cloned()
            // Matching REVM's EmptyDB for now
            .unwrap_or_else(|| keccak256(number.to_string().as_bytes())))
    }
}
