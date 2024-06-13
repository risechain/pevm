// A storage that stores data in memory.

use std::fmt::Debug;

use ahash::AHashMap;
use alloy_primitives::{keccak256, Address, Bytes, B256, U256};

use crate::{AccountBasic, EvmAccount, Storage};

/// Fetch state data via RPC to execute.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    accounts: AHashMap<Address, EvmAccount>,
    block_hashes: AHashMap<U256, B256>,
}

impl InMemoryStorage {
    /// Create a new InMemoryStorage
    pub fn new(
        accounts: impl IntoIterator<Item = (Address, impl Into<EvmAccount>)>,
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
    pub fn insert_account(&mut self, address: Address, account: EvmAccount) {
        self.accounts.insert(address, account);
    }
}

impl Storage for InMemoryStorage {
    // TODO: More proper error handling
    type Error = ();

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .map(|account| account.basic.clone()))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Bytes, Self::Error> {
        for account in self.accounts.values() {
            if account
                .basic
                .code_hash
                .is_some_and(|hash| &hash == code_hash)
            {
                return Ok(account.basic.code.clone());
            }
        }
        Ok(Bytes::default())
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .is_some_and(|account| !account.storage.is_empty()))
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .and_then(|account| account.storage.get(index))
            .cloned()
            .unwrap_or_default())
    }

    fn block_hash(&self, number: &U256) -> Result<B256, Self::Error> {
        Ok(self
            .block_hashes
            .get(number)
            .cloned()
            // Matching REVM's EmptyDB for now
            .unwrap_or_else(|| keccak256(number.to_string().as_bytes())))
    }
}
