//! Convert a block folder to a MDBX folder
//! For help, run: `cargo run --example to_mdbx -- --help`

use alloy_primitives::{Address, Bytes, FixedBytes, B256, B64, U64};
use clap::Parser;
use libmdbx::{
    Database, DatabaseKind, DatabaseOptions, Mode, NoWriteMap, ReadWriteOptions, SyncMode,
    TableFlags, WriteFlags,
};
use pevm::{EvmAccount, EvmCode};
use revm::primitives::Bytecode;
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufReader, Error},
    path::Path,
};

type B416 = FixedBytes<52>;
type B0 = FixedBytes<0>;

/// Convert a block folder to a MDBX folder
#[derive(Parser, Debug)]
#[clap(name = "to_mdbx")]
struct Args {
    /// Path to bytecodes.bincode
    #[clap(long, value_name = "FILE")]
    bytecodes: String,
    /// Path to pre_state.json
    #[clap(long, value_name = "FILE")]
    pre_state: String,
    /// Path to block_hashes.json
    #[clap(long, value_name = "FILE")]
    block_hashes: Option<String>,
    /// Path to output MDBX dir
    #[clap(long, value_name = "DIR")]
    output: String,
}

#[derive(Debug, Clone, Default)]
struct Tables {
    balance: HashMap<Address, B256>,
    nonce: HashMap<Address, B64>,
    code_hash: HashMap<Address, B256>,
    code_by_hash: HashMap<B256, Bytes>,
    has_storage: HashMap<Address, B0>,
    storage: HashMap<B416, B256>,
    block_hash: HashMap<B64, B256>,
}

fn open_db(dir: impl AsRef<Path>) -> Result<Database<NoWriteMap>, libmdbx::Error> {
    const MB: isize = 1048576;

    #[allow(clippy::identity_op)]
    Database::<NoWriteMap>::open_with_options(
        dir.as_ref(),
        DatabaseOptions {
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
            ..DatabaseOptions::default()
        },
    )
}

fn read_tables_from(
    path_to_bytecodes_file: impl AsRef<Path>,
    path_to_pre_state_file: impl AsRef<Path>,
    path_to_block_hash_file: Option<impl AsRef<Path>>,
) -> Result<Tables, Error> {
    let mut result = Tables::default();

    let bytecodes: BTreeMap<B256, EvmCode> = {
        let file = File::open(path_to_bytecodes_file)?;
        bincode::deserialize_from(BufReader::new(file)).map_err(Error::other)?
    };
    for (code_hash, evm_code) in bytecodes {
        let bytes = Bytecode::from(evm_code).original_bytes();
        result.code_by_hash.insert(code_hash, bytes);
    }

    let state: HashMap<Address, EvmAccount> = {
        let file = File::open(path_to_pre_state_file)?;
        serde_json::from_reader(BufReader::new(file))?
    };
    for (address, account) in state {
        assert!(account.code.is_none());
        result.balance.insert(address, account.basic.balance.into());
        result.nonce.insert(address, account.basic.nonce.into());
        if let Some(code_hash) = account.code_hash {
            result.code_hash.insert(address, code_hash);
        }
        if !account.storage.is_empty() {
            result.has_storage.insert(address, B0::default());
            for (index, storage_value) in account.storage {
                result.storage.insert(
                    B416::from_slice(&[address.as_slice(), B256::from(index).as_slice()].concat()),
                    storage_value.into(),
                );
            }
        }
    }

    if let Some(path) = path_to_block_hash_file {
        let file = File::open(path)?;
        let block_hashes: HashMap<U64, B256> = serde_json::from_reader(BufReader::new(file))?;
        result.block_hash = block_hashes
            .into_iter()
            .map(|(k, v)| (B64::from(k), v))
            .collect()
    }

    Ok(result)
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

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let tables = read_tables_from(args.bytecodes, args.pre_state, args.block_hashes)?;
    let db = open_db(args.output).map_err(Error::other)?;
    write_table_to(&db, "balance", tables.balance.iter()).map_err(Error::other)?;
    write_table_to(&db, "nonce", tables.nonce.iter()).map_err(Error::other)?;
    write_table_to(&db, "code_hash", tables.code_hash.iter()).map_err(Error::other)?;
    write_table_to(&db, "code_by_hash", tables.code_by_hash.iter()).map_err(Error::other)?;
    write_table_to(&db, "has_storage", tables.has_storage.iter()).map_err(Error::other)?;
    write_table_to(&db, "storage", tables.storage.iter()).map_err(Error::other)?;
    write_table_to(&db, "block_hash", tables.block_hash.iter()).map_err(Error::other)?;

    Ok(())
}
