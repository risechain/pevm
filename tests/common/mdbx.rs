use std::{collections::HashMap, path::Path};

use alloy_primitives::{b256, Address, B256, B64, U256};
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

/// The Keccak-256 hash of the empty string `""`.
const KECCAK_EMPTY: B256 =
    b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
type PackedAccount = (B256, U256, u64);

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

    // balance, nonce, code_hash (default: KECCAK_EMPTY)
    let mut encoded_accounts = HashMap::<Address, PackedAccount>::new();
    let mut storage = HashMap::<(Address, B256), B256>::new();
    for (&address, account) in pre_state {
        encoded_accounts.insert(
            address,
            (
                account.code_hash.unwrap_or(KECCAK_EMPTY),
                account.balance,
                account.nonce,
            ),
        );
        for (&index, &storage_value) in account.storage.iter() {
            storage.insert((address, B256::from(index)), B256::from(storage_value));
        }
    }

    write_table_to(
        &env,
        "encoded_accounts",
        encoded_accounts.into_iter().map(|(address, (c, b, n))| {
            (
                address,
                [
                    c.as_slice(),
                    B256::from(b).as_slice(),
                    B64::from(n).as_slice(),
                ]
                .concat(),
            )
        }),
    )?;

    write_table_to(
        &env,
        "storage",
        storage
            .into_iter()
            .map(|((a, i), v)| ([a.as_slice(), i.as_slice()].concat(), v)),
    )?;

    // write_table_to(
    //     &env,
    //     "accounts",
    //     pre_state.map(|(address, account)| (address, bincode::serialize(account).unwrap())),
    // )?;

    write_table_to(
        &env,
        "block_hashes",
        block_hashes
            .map(|(block_number, block_hash)| (Into::<B64>::into(*block_number), block_hash)),
    )?;
    Ok(())
}
