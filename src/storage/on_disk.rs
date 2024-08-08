use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use ahash::AHashMap;
use alloy_primitives::{keccak256, Address, B256, B64, U256};
use libmdbx::{
    Database, DatabaseKind, DatabaseOptions, Mode, NoWriteMap, ReadWriteOptions, SyncMode,
    TableFlags, WriteFlags,
};

use super::{AccountBasic, EvmAccount, EvmCode, Storage};

/// A storage that reads data from an on-disk MDBX database.
#[derive(Debug)]
pub struct OnDiskStorage {
    db: Database<NoWriteMap>,
    cache_accounts: Mutex<AHashMap<Address, Option<EvmAccount>>>,
}

impl OnDiskStorage {
    /// Opens the on-disk storage at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, libmdbx::Error> {
        let db = Database::open_with_options(
            &path,
            DatabaseOptions {
                max_tables: Some(16),
                ..DatabaseOptions::default()
            },
        )?;
        Ok(Self {
            db,
            cache_accounts: Mutex::default(),
        })
    }

    fn load_account_and_then<T, F: Fn(&Option<EvmAccount>) -> Result<T, libmdbx::Error>>(
        &self,
        address: &Address,
        op: F,
    ) -> Result<T, libmdbx::Error> {
        if let Some(account) = self.cache_accounts.lock().unwrap().get(address) {
            return op(account);
        }

        let tx = self.db.begin_ro_txn()?;
        let table = tx.open_table(Some("accounts"))?;
        let bytes: Option<Vec<u8>> = tx.get(&table, address.as_ref())?;
        let account: Option<EvmAccount> = match bytes {
            Some(bytes) => Some(
                bincode::deserialize(bytes.as_slice())
                    .map_err(|err| libmdbx::Error::DecodeError(err))?,
            ),
            None => None,
        };
        let result = op(&account);

        self.cache_accounts
            .lock()
            .unwrap()
            .insert(*address, account);

        result
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        self.cache_accounts.lock().unwrap().clear()
    }
}

impl Storage for OnDiskStorage {
    type Error = libmdbx::Error;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        self.load_account_and_then(address, |account| {
            Ok(account.as_ref().map(|account| AccountBasic {
                balance: account.balance,
                nonce: account.nonce,
            }))
        })
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        self.load_account_and_then(address, |account| {
            Ok(account.as_ref().and_then(|account| account.code_hash))
        })
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        let tx = self.db.begin_ro_txn()?;
        let table = tx.open_table(Some("bytecodes"))?;
        let bytes: Option<Vec<u8>> = tx.get(&table, code_hash.as_ref())?;
        let code: Option<EvmCode> = match bytes {
            Some(bytes) => Some(
                bincode::deserialize(bytes.as_slice())
                    .map_err(|err| libmdbx::Error::DecodeError(err))?,
            ),
            None => None,
        };
        Ok(code)
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        self.load_account_and_then(address, |account| {
            Ok(account
                .as_ref()
                .map(|account| !account.storage.is_empty())
                .unwrap_or_default())
        })
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        self.load_account_and_then(address, |account| {
            Ok(account
                .as_ref()
                .and_then(|account| account.storage.get(index).cloned())
                .unwrap_or_default())
        })
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        let tx = self.db.begin_ro_txn()?;
        let Some(block_hash) = tx
            .open_table(Some("block_hashes"))
            .and_then(|table| tx.get(&table, B64::from(*number).as_ref()))
            .map(|bytes: Option<[u8; 32]>| bytes.map(B256::from))?
        else {
            return Ok(keccak256(number.to_string().as_bytes()));
        };
        Ok(block_hash)
    }
}

const MB: isize = 1048576;

#[allow(clippy::identity_op)]
const DEFAULT_DB_OPTIONS: DatabaseOptions = DatabaseOptions {
    max_tables: Some(16),
    mode: Mode::ReadWrite(ReadWriteOptions {
        // https://erthink.github.io/libmdbx/group__c__settings.html#ga79065e4f3c5fb2ad37a52b59224d583e
        // https://github.com/erthink/libmdbx/issues/136#issuecomment-727490550
        sync_mode: SyncMode::Durable,
        min_size: Some(1 * MB), // The lower bound allows you to prevent database shrinking below certain reasonable size to avoid unnecessary resizing costs.
        max_size: Some(1024 * MB), // The upper bound allows you to prevent database growth above certain reasonable size.
        growth_step: Some(1 * MB), // The growth step must be greater than zero to allow the database to grow, but also reasonable not too small, since increasing the size by little steps will result a large overhead.
        shrink_threshold: Some(4 * MB), // The shrink threshold must be greater than zero to allow the database to shrink but also reasonable not too small (to avoid extra overhead) and not less than growth step to avoid up-and-down flouncing.
    }),
    permissions: None,
    max_readers: None,
    rp_augment_limit: None,
    loose_limit: None,
    dp_reserve_limit: None,
    txn_dp_limit: None,
    spill_max_denominator: None,
    spill_min_denominator: None,
    page_size: None,
    no_sub_dir: false,
    exclusive: false,
    accede: false,
    no_rdahead: false,
    no_meminit: false,
    coalesce: false,
    liforeclaim: false,
};

fn open_db(dir: impl AsRef<Path>) -> Result<Database<NoWriteMap>, libmdbx::Error> {
    Database::<NoWriteMap>::open_with_options(dir.as_ref(), DEFAULT_DB_OPTIONS)
}

fn write_table_to<E: DatabaseKind, K: AsRef<[u8]>, V: AsRef<[u8]>>(
    db: &Database<E>,
    table_name: &str,
    entries: impl Iterator<Item = (K, V)>,
) -> Result<(), libmdbx::Error> {
    let tx = db.begin_rw_txn()?;
    let table = tx.create_table(Some(table_name), TableFlags::default())?;
    for (k, v) in entries {
        tx.put(&table, k, v, WriteFlags::UPSERT)?;
    }
    tx.commit()?;
    Ok(())
}

/// Create a temp dir containing MDBX
pub fn create_db_dir<'a>(
    block_number: &str,
    bytecodes: impl Iterator<Item = (&'a B256, &'a EvmCode)>,
    pre_state: impl Iterator<Item = (&'a Address, &'a EvmAccount)>,
    block_hashes: impl Iterator<Item = (&'a u64, &'a B256)>,
) -> Result<PathBuf, libmdbx::Error> {
    let mut dir = std::env::temp_dir();
    dir.push(block_number);

    let db = open_db(&dir)?;
    write_table_to(
        &db,
        "bytecodes",
        bytecodes.map(|(code_hash, evm_code)| (code_hash, bincode::serialize(&evm_code).unwrap())),
    )?;
    write_table_to(
        &db,
        "accounts",
        pre_state.map(|(address, account)| (address, bincode::serialize(account).unwrap())),
    )?;
    write_table_to(
        &db,
        "block_hashes",
        block_hashes
            .map(|(block_number, block_hash)| (Into::<B64>::into(*block_number), block_hash)),
    )?;

    Ok(dir)
}

/// Remove a temp dir
pub fn remove_db_dir(db_dir: PathBuf) -> Result<(), std::io::Error> {
    std::fs::remove_dir_all(&db_dir)
}
