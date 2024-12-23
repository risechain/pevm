use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::{keccak256, Address, B256, U256};

use super::{BlockHashes, Bytecodes, ChainState, EvmCode};
use crate::{AccountBasic, Storage};

/// A storage that stores chain data in memory.
#[derive(Debug, Clone, Default)]
pub struct InMemoryStorage {
    accounts: ChainState,
    bytecodes: Arc<Bytecodes>,
    block_hashes: Arc<BlockHashes>,
}

impl InMemoryStorage {
    /// Construct a new [`InMemoryStorage`]
    pub const fn new(
        accounts: ChainState,
        bytecodes: Arc<Bytecodes>,
        block_hashes: Arc<BlockHashes>,
    ) -> Self {
        Self {
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
        Ok(self.accounts.get(address).map(|account| AccountBasic {
            balance: account.balance,
            nonce: account.nonce,
        }))
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
            .copied()
            .unwrap_or_default())
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        Ok(self
            .block_hashes
            .get(number)
            .copied()
            // Matching REVM's [EmptyDB] for now
            .unwrap_or_else(|| keccak256(number.to_string().as_bytes())))
    }
}
