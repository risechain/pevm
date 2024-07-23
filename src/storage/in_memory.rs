use std::{collections::HashMap, fmt::Debug};

use ahash::AHashMap;
use alloy_primitives::{keccak256, Address, B256, U256};

use super::EvmCode;
use crate::{AccountBasic, BuildAddressHasher, EvmAccount, Storage};

type Accounts = HashMap<Address, EvmAccount, BuildAddressHasher>;

/// A storage that stores chain data in memory.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    accounts: Accounts,
    bytecodes: AHashMap<B256, EvmCode>,
    block_hashes: AHashMap<u64, B256>,
}

impl InMemoryStorage {
    /// Construct a new [InMemoryStorage]
    // TODO: Take in [bytecodes] instead of reading duplicates from
    // [accounts].
    pub fn new(
        accounts: impl IntoIterator<Item = (Address, EvmAccount)>,
        block_hashes: impl IntoIterator<Item = (u64, B256)>,
    ) -> Self {
        let mut result = Self {
            accounts: Accounts::default(),
            bytecodes: AHashMap::new(),
            block_hashes: block_hashes.into_iter().collect(),
        };

        for (address, mut account) in accounts {
            let code_hash: Option<B256> = account.code_hash;
            let code: Option<EvmCode> = account.code.take();
            result.accounts.insert(address, account);
            if let Some(code_hash) = code_hash {
                result.bytecodes.insert(code_hash, code.unwrap_or_default());
            }
        }

        result
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

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        Ok(self
            .accounts
            .get(address)
            .and_then(|account| account.code_hash))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        Ok(self.bytecodes.get(code_hash).cloned())
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
