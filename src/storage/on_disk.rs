use std::path::Path;

use alloy_primitives::{keccak256, Address, B256, B64, U256};
use dashmap::{DashMap, Entry, OccupiedEntry};
use libmdbx::{Database, DatabaseOptions, NoWriteMap};

use super::{AccountBasic, EvmAccount, EvmCode, Storage};

/// A storage that reads data from an on-disk MDBX database.
#[derive(Debug)]
pub struct OnDiskStorage {
    db: Database<NoWriteMap>,
    cache_accounts: DashMap<Address, Option<EvmAccount>>,
    cache_bytecodes: DashMap<B256, Option<EvmCode>>,
    cache_block_hashes: DashMap<u64, B256>,
}

impl OnDiskStorage {
    /// Opens the on-disk storage at the specified path.
    pub fn open(path: impl AsRef<Path>, options: DatabaseOptions) -> Result<Self, libmdbx::Error> {
        let db = Database::open_with_options(path, options)?;
        Ok(Self {
            db,
            cache_accounts: DashMap::default(),
            cache_bytecodes: DashMap::default(),
            cache_block_hashes: DashMap::default(),
        })
    }

    fn load_account(
        &self,
        address: Address,
    ) -> Result<OccupiedEntry<Address, Option<EvmAccount>>, libmdbx::Error> {
        match self.cache_accounts.entry(address) {
            Entry::Occupied(occupied) => Ok(occupied),
            Entry::Vacant(vacant) => {
                let tx = self.db.begin_ro_txn()?;
                let table = tx.open_table(Some("accounts"))?;
                let bytes: Option<Vec<u8>> = tx.get(&table, address.as_ref())?;
                drop(tx);
                let account: Option<EvmAccount> = match bytes {
                    Some(bytes) => Some(
                        bincode::deserialize(bytes.as_slice())
                            .map_err(|err| libmdbx::Error::DecodeError(err))?,
                    ),
                    None => None,
                };
                Ok(vacant.insert_entry(account))
            }
        }
    }
}

impl Storage for OnDiskStorage {
    type Error = libmdbx::Error;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        let entry = self.load_account(*address)?;
        Ok(entry.get().as_ref().map(|account| AccountBasic {
            balance: account.balance,
            nonce: account.nonce,
        }))
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        let entry = self.load_account(*address)?;
        Ok(entry.get().as_ref().and_then(|account| account.code_hash))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        match self.cache_bytecodes.entry(*code_hash) {
            Entry::Occupied(occupied) => Ok(occupied.get().clone()),
            Entry::Vacant(vacant) => {
                let tx = self.db.begin_ro_txn()?;
                let table = tx.open_table(Some("bytecodes"))?;
                let bytes: Option<Vec<u8>> = tx.get(&table, code_hash.as_ref())?;
                drop(tx);
                let code: Option<EvmCode> = match bytes {
                    Some(bytes) => Some(
                        bincode::deserialize(bytes.as_slice())
                            .map_err(|err| libmdbx::Error::DecodeError(err))?,
                    ),
                    None => None,
                };
                vacant.insert(code.clone());
                Ok(code)
            }
        }
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        let entry = self.load_account(*address)?;
        Ok(entry
            .get()
            .as_ref()
            .map(|account| !account.storage.is_empty())
            .unwrap_or_default())
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        let entry = self.load_account(*address)?;
        Ok(entry
            .get()
            .as_ref()
            .and_then(|account| account.storage.get(index).copied())
            .unwrap_or_default())
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        match self.cache_block_hashes.entry(*number) {
            Entry::Occupied(occupied) => Ok(*occupied.get()),
            Entry::Vacant(vacant) => {
                let tx = self.db.begin_ro_txn()?;
                let table = tx.open_table(Some("block_hashes"))?;
                let bytes: Option<[u8; 32]> = tx.get(&table, B64::from(*number).as_ref())?;
                drop(tx);
                let block_hash = match bytes {
                    Some(bytes) => B256::from(bytes),
                    None => keccak256(number.to_string().as_bytes()),
                };
                vacant.insert(block_hash);
                Ok(block_hash)
            }
        }
    }
}
