use std::collections::HashMap;

use revm::{
    db::DbAccount,
    primitives::{Address, Bytecode, B256, U256},
};

use crate::{MemoryLocation, MemoryValue, ReadError};

// TODO: Populate the remaining missing pieces like logs, etc.
pub(crate) struct Storage {
    accounts: HashMap<Address, DbAccount>,
    contracts: HashMap<B256, Bytecode>,
    block_hashes: HashMap<U256, B256>,
}

impl Storage {
    pub(crate) fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            contracts: HashMap::new(),
            block_hashes: HashMap::new(),
        }
    }

    pub(crate) fn read_memory_location(
        &self,
        location: &MemoryLocation,
    ) -> Result<MemoryValue, ReadError> {
        match location {
            MemoryLocation::Basic(address) => Ok(MemoryValue::Basic(
                self.accounts.get(address).map(|a| a.info.clone()),
            )),
            MemoryLocation::Storage((address, index)) => Ok(MemoryValue::Storage(
                self.accounts
                    .get(address)
                    .and_then(|a| a.storage.get(index))
                    .cloned()
                    .unwrap_or(U256::ZERO),
            )),
        }
    }

    pub(crate) fn code_by_hash(&self, code_hash: &B256) -> Option<Bytecode> {
        self.contracts.get(code_hash).cloned()
    }

    pub(crate) fn block_hash(&self, number: &U256) -> Option<B256> {
        self.block_hashes.get(number).cloned()
    }
}
