use std::{collections::HashMap, fmt::Debug};

use ahash::AHashMap;
use alloy_primitives::{keccak256, Address, B256, U256};

use super::EvmCode;
use crate::{AccountBasic, BuildAddressHasher, EvmAccount, Storage};

/// A storage that stores chain data in memory.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    accounts: HashMap<Address, EvmAccount, BuildAddressHasher>,
    block_hashes: AHashMap<u64, B256>,
}

impl InMemoryStorage {
    /// Construct a new [InMemoryStorage]
    pub fn new(
        accounts: impl IntoIterator<Item = (Address, EvmAccount)>,
        block_hashes: impl IntoIterator<Item = (u64, B256)>,
    ) -> Self {
        InMemoryStorage {
            accounts: accounts.into_iter().collect(),
            block_hashes: block_hashes.into_iter().collect(),
        }
    }
}

impl Storage for InMemoryStorage {
    // TODO: More proper error handling
    type Error = u8;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .map(|account| account.basic.clone()))
    }

    fn code_by_address(&self, address: &Address) -> Result<Option<EvmCode>, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .and_then(|account| account.code.clone()))
    }

    // TODO: Map [B256] to [EvmCode] for much faster search
    // Currently acceptable as this is not used during execution.
    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        for account in self.accounts.values() {
            if account
                .basic
                .code_hash
                .is_some_and(|hash| &hash == code_hash)
            {
                return Ok(account.code.clone());
            }
        }
        Ok(None)
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

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        Ok(self
            .block_hashes
            .get(number)
            .cloned()
            // Matching REVM's [EmptyDB] for now
            .unwrap_or_else(|| keccak256(number.to_string().as_bytes())))
    }
}
