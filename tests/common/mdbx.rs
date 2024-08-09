use std::path::Path;

use alloy_primitives::{Address, B256, B64};
use libmdbx::{
    Database, DatabaseKind, DatabaseOptions, Mode, NoWriteMap, ReadWriteOptions, SyncMode,
    TableFlags, WriteFlags,
};
use pevm::{EvmAccount, EvmCode};

const MB: isize = 1048576;

#[allow(clippy::identity_op)]
const DEFAULT_DB_OPTIONS: DatabaseOptions = DatabaseOptions {
    max_tables: Some(16), // We need more tables than the default limit (1).
    mode: Mode::ReadWrite(ReadWriteOptions {
        // We need to config storage options to avoid MDBX_MAP_FULL.
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
pub(crate) fn create_db_dir<'a>(
    dir: impl AsRef<Path>,
    bytecodes: impl Iterator<Item = (&'a B256, &'a EvmCode)>,
    pre_state: impl Iterator<Item = (&'a Address, &'a EvmAccount)>,
    block_hashes: impl Iterator<Item = (&'a u64, &'a B256)>,
) -> Result<(), libmdbx::Error> {
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
    Ok(())
}
