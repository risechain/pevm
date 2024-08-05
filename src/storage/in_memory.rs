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
        bytecodes: impl IntoIterator<Item = (B256, EvmCode)>,
        block_hashes: impl IntoIterator<Item = (u64, B256)>,
    ) -> Self {
        let mut accounts: Accounts = accounts.into_iter().collect();
        let mut bytecodes: AHashMap<B256, EvmCode> = bytecodes.into_iter().collect();
        let block_hashes: AHashMap<u64, B256> = block_hashes.into_iter().collect();

        for account in accounts.values_mut() {
            let code = account.code.take();
            if let (Some(code), Some(code_hash)) = (code, account.code_hash) {
                bytecodes.insert(code_hash, code);
            }
        }

        InMemoryStorage {
            accounts,
            bytecodes,
            block_hashes,
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
