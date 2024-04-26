use std::collections::HashMap;

use revm::primitives::{Account, AccountInfo, Address, Bytecode, B256, U256};

use crate::ReadError;

/// An interface to provide chain state to BlockSTM for transaction execution.
/// TODO: Populate the remaining missing pieces like logs, etc.
/// TODO: Better API for third-pary integration.
#[derive(Debug, Default, Clone)]
pub struct Storage {
    /// A map of account addresses to their corresponding account information.
    pub accounts: HashMap<Address, Account>,
    /// A map of contract hashes to their corresponding bytecode.
    pub contracts: HashMap<B256, Bytecode>,
    /// A map of block numbers to their corresponding block hashes.
    pub block_hashes: HashMap<U256, B256>,
}

impl Storage {
    /// Initialize a storage.
    /// TODO: Init the storage with a custom genesis state.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            contracts: HashMap::new(),
            block_hashes: HashMap::new(),
        }
    }

    /// Insert a new account into the storage. Useful for mocking.
    pub fn insert_account_info(&mut self, address: Address, info: AccountInfo) {
        self.accounts.insert(address, Account::from(info));
    }

    /// Get the basic account information for a given address.
    pub fn basic(&self, address: Address) -> Result<AccountInfo, ReadError> {
        match self.accounts.get(&address) {
            Some(account) => Ok(account.info.clone()),
            None => Err(ReadError::NotFound),
        }
    }

    /// Get the bytecode for a contract given its code hash.
    pub(crate) fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, ReadError> {
        match self.contracts.get(&code_hash) {
            Some(byte_code) => Ok(byte_code.clone()),
            None => Err(ReadError::NotFound),
        }
    }

    /// Get the storage value at a specific index for a given address.
    pub(crate) fn storage(&self, address: Address, index: U256) -> Result<U256, ReadError> {
        Ok(self
            .accounts
            .get(&address)
            .and_then(|a| a.storage.get(&index).map(|s| s.present_value))
            .unwrap_or(U256::ZERO))
    }

    /// Get the block hash for a given block number.
    pub(crate) fn block_hash(&self, number: U256) -> Result<B256, ReadError> {
        match self.block_hashes.get(&number) {
            Some(block_hash) => Ok(*block_hash),
            None => Err(ReadError::NotFound),
        }
    }
}
