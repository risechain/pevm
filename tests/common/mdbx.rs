use std::path::Path;

use alloy_primitives::{Address, B256, B64};
use pevm::{EvmAccount, EvmCode};
use reth_libmdbx::{
    DatabaseFlags, Environment, EnvironmentFlags, Geometry, Mode, SyncMode, WriteFlags,
};

const MB: usize = 1048576;

fn open_env(dir: impl AsRef<Path>) -> Result<Environment, reth_libmdbx::Error> {
    Environment::builder()
        .set_max_dbs(16)
        .set_flags(EnvironmentFlags::from(Mode::ReadWrite {
            sync_mode: SyncMode::Durable,
        }))
        .set_geometry(Geometry {
            size: Some(0..512 * MB),
            growth_step: Some(2 * MB as isize),
            shrink_threshold: Some(8 * MB as isize),
            page_size: None,
        })
        .open(dir.as_ref())
}

fn write_table_to<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    env: &Environment,
    table_name: &str,
    entries: impl Iterator<Item = (K, V)>,
) -> Result<(), reth_libmdbx::Error> {
    let tx = env.begin_rw_txn()?;
    let table = tx.create_db(Some(table_name), DatabaseFlags::default())?;
    for (k, v) in entries {
        tx.put(table.dbi(), k, v, WriteFlags::UPSERT)?;
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
) -> Result<(), reth_libmdbx::Error> {
    let env = open_env(&dir)?;
    write_table_to(
        &env,
        "bytecodes",
        bytecodes.map(|(code_hash, evm_code)| (code_hash, bincode::serialize(&evm_code).unwrap())),
    )?;
    write_table_to(
        &env,
        "accounts",
        pre_state.map(|(address, account)| (address, bincode::serialize(account).unwrap())),
    )?;
    write_table_to(
        &env,
        "block_hashes",
        block_hashes
            .map(|(block_number, block_hash)| (Into::<B64>::into(*block_number), block_hash)),
    )?;
    Ok(())
}
