use std::{path::Path, thread::ThreadId};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::{keccak256, Address, FixedBytes, B256, B64, U256};
use dashmap::{DashMap, Entry, OccupiedEntry};
use reth_libmdbx::{Database, Environment, Transaction, RO};

use super::{AccountBasic, EvmCode, Storage};

/// A storage that reads data from an on-disk MDBX database.
#[derive(Debug)]
pub struct OnDiskStorage {
    env: Environment,
    cache_encoded_accounts: DashMap<Address, Option<(B256, B64, B256)>>,
    cache_storage: DashMap<(Address, U256), U256>,
    cache_bytecodes: DashMap<B256, Option<EvmCode>>,
    cache_block_hashes: DashMap<u64, B256>,
    cache_txs: DashMap<ThreadId, Transaction<RO>>,
    table_encoded_accounts: Database,
    table_storage: Database,
    table_bytecodes: Database,
    table_block_hashes: Database,
}

impl OnDiskStorage {
    /// Opens the on-disk storage at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, reth_libmdbx::Error> {
        let env = Environment::builder().set_max_dbs(16).open(path.as_ref())?;
        let tx = env.begin_ro_txn()?;
        // let table_accounts = tx.open_db(Some("accounts"))?;
        let table_encoded_accounts = tx.open_db(Some("encoded_accounts"))?;
        let table_storage = tx.open_db(Some("storage"))?;
        let table_bytecodes = tx.open_db(Some("bytecodes"))?;
        let table_block_hashes = tx.open_db(Some("block_hashes"))?;
        Ok(Self {
            env,
            cache_encoded_accounts: DashMap::default(),
            cache_storage: DashMap::default(),
            cache_bytecodes: DashMap::default(),
            cache_block_hashes: DashMap::default(),
            cache_txs: DashMap::default(),
            table_encoded_accounts,
            table_storage,
            table_bytecodes,
            table_block_hashes,
        })
    }

    fn load_encoded_account(
        &self,
        address: Address,
    ) -> Result<OccupiedEntry<Address, Option<(B256, B64, B256)>>, reth_libmdbx::Error> {
        match self.cache_encoded_accounts.entry(address) {
            Entry::Occupied(occupied) => Ok(occupied),
            Entry::Vacant(vacant) => {
                let tx_ref = self
                    .cache_txs
                    .entry(std::thread::current().id())
                    .or_insert_with(|| self.env.begin_ro_txn().unwrap());
                let bytes: Option<[u8; 32 + 8 + 32]> = tx_ref
                    .value()
                    .get(self.table_encoded_accounts.dbi(), address.as_ref())?;
                drop(tx_ref);

                let decoded = bytes.map(|bytes| {
                    let b = B256::from_slice(&bytes[0..32]);
                    let n = B64::from_slice(&bytes[32..(32 + 8)]);
                    let c = B256::from_slice(&bytes[(32 + 8)..(32 + 8 + 32)]);
                    (b, n, c)
                });
                Ok(vacant.insert_entry(decoded))
            }
        }
    }
}

impl Storage for OnDiskStorage {
    type Error = reth_libmdbx::Error;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        let entry = self.load_encoded_account(*address)?;
        Ok(entry.get().as_ref().map(|account| AccountBasic {
            balance: account.0.into(),
            nonce: account.1.into(),
        }))
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        let entry = self.load_encoded_account(*address)?;
        Ok(entry
            .get()
            .as_ref()
            .and_then(|(_, _, c)| (*c != KECCAK_EMPTY).then_some(*c)))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        match self.cache_bytecodes.entry(*code_hash) {
            Entry::Occupied(occupied) => Ok(occupied.get().clone()),
            Entry::Vacant(vacant) => {
                let tx_ref = self
                    .cache_txs
                    .entry(std::thread::current().id())
                    .or_insert_with(|| self.env.begin_ro_txn().unwrap());
                let bytes: Option<Vec<u8>> = tx_ref
                    .value()
                    .get(self.table_bytecodes.dbi(), code_hash.as_ref())?;
                drop(tx_ref);
                let code: Option<EvmCode> = match bytes {
                    Some(bytes) => Some(
                        bincode::deserialize(bytes.as_slice())
                            .map_err(|_| reth_libmdbx::Error::DecodeError)?,
                    ),
                    None => None,
                };
                vacant.insert(code.clone());
                Ok(code)
            }
        }
    }

    fn has_storage(&self, _address: &Address) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        match self.cache_storage.entry((*address, *index)) {
            Entry::Occupied(occupied) => Ok(*occupied.get()),
            Entry::Vacant(vacant) => {
                let tx_ref = self
                    .cache_txs
                    .entry(std::thread::current().id())
                    .or_insert_with(|| self.env.begin_ro_txn().unwrap());
                let bytes: Option<[u8; 32]> = tx_ref.value().get(
                    self.table_storage.dbi(),
                    FixedBytes::<{ 20 + 32 }>::from_slice(
                        &[address.as_slice(), Into::<B256>::into(*index).as_slice()].concat(),
                    )
                    .as_ref(),
                )?;
                drop(tx_ref);
                let storage_value = match bytes {
                    Some(bytes) => U256::from_be_bytes(bytes),
                    None => U256::ZERO,
                };
                vacant.insert(storage_value);
                Ok(storage_value)
            }
        }
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        match self.cache_block_hashes.entry(*number) {
            Entry::Occupied(occupied) => Ok(*occupied.get()),
            Entry::Vacant(vacant) => {
                let tx_ref = self
                    .cache_txs
                    .entry(std::thread::current().id())
                    .or_insert_with(|| self.env.begin_ro_txn().unwrap());
                let bytes: Option<[u8; 32]> = tx_ref
                    .value()
                    .get(self.table_block_hashes.dbi(), B64::from(*number).as_ref())?;
                drop(tx_ref);
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
