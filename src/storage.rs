use std::collections::HashMap;

use revm::{
    db::DbAccount,
    primitives::{AccountInfo, Address, Bytecode, B256, U256},
    DatabaseRef,
};

use crate::ReadError;

// TODO: Populate the remaining missing pieces like logs, etc.
pub struct Storage {
    accounts: HashMap<Address, DbAccount>,
    contracts: HashMap<B256, Bytecode>,
    block_hashes: HashMap<U256, B256>,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            contracts: HashMap::new(),
            block_hashes: HashMap::new(),
        }
    }
}

impl DatabaseRef for Storage {
    type Error = ReadError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // TODO: Properly return NotFound here.
        Ok(self.accounts.get(&address).and_then(|a| a.info()))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(byte_code) => Ok(byte_code.clone()),
            None => Err(ReadError::NotFound),
        }
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self
            .accounts
            .get(&address)
            .and_then(|a| a.storage.get(&index))
            .cloned()
            .unwrap_or(U256::ZERO))
    }

    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        match self.block_hashes.get(&number) {
            Some(block_hash) => Ok(block_hash.clone()),
            None => Err(ReadError::NotFound),
        }
    }
}
