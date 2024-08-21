use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};

use ahash::AHashMap;
use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types::{Block, Header};
use pevm::{Bytecodes, EvmAccount, InMemoryStorage};

pub mod runner;
pub use runner::{assert_execution_result, mock_account, test_execute_alloy, test_execute_revm};
mod mdbx;
pub mod storage;

pub type ChainState = AHashMap<Address, EvmAccount>;
pub type BlockHashes = AHashMap<u64, B256>;

pub static MOCK_ALLOY_BLOCK_HEADER: Header = Header {
    // Minimal requirements for execution
    number: Some(1),
    timestamp: 1710338135,
    mix_hash: Some(B256::ZERO),
    excess_blob_gas: Some(0),
    gas_limit: u128::MAX,
    // Defaults
    hash: None,
    parent_hash: B256::ZERO,
    uncles_hash: B256::ZERO,
    miner: Address::ZERO,
    state_root: B256::ZERO,
    transactions_root: B256::ZERO,
    receipts_root: B256::ZERO,
    logs_bloom: Bloom::ZERO,
    difficulty: U256::ZERO,
    gas_used: 0,
    total_difficulty: Some(U256::ZERO),
    extra_data: Bytes::new(),
    nonce: None,
    base_fee_per_gas: None,
    withdrawals_root: None,
    blob_gas_used: None,
    parent_beacon_block_root: None,
    requests_root: None,
};

pub const RAW_TRANSFER_GAS_LIMIT: u64 = 21_000;

fn get_block_paths() -> Result<Vec<PathBuf>, std::io::Error> {
    let dir = fs::read_dir("data/blocks")?;
    let mut entries = dir.into_iter().collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| {
        entry
            .file_name()
            .into_string()
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or_default()
    });
    Ok(entries.into_iter().map(|entry| entry.path()).collect())
}

// TODO: Put somewhere better?
pub fn for_each_block_from_disk(mut handler: impl FnMut(Block, InMemoryStorage, &PathBuf)) {
    // Parse bytecodes
    let bytecodes: Bytecodes = bincode::deserialize_from(BufReader::new(
        File::open("data/bytecodes.bincode").unwrap(),
    ))
    .unwrap();

    let mdbx_dir = std::env::temp_dir().join("mdbx");
    mdbx::init_db_dir(&mdbx_dir, bytecodes.iter()).unwrap();

    for block_path in get_block_paths().unwrap() {
        // Parse block
        let block: Block = serde_json::from_reader(BufReader::new(
            File::open(block_path.join("block.json")).unwrap(),
        ))
        .unwrap();

        let txs = block.transactions.as_transactions().unwrap_or_default();
        let raw_count = txs.iter().filter(|tx| tx.input.is_empty()).count();
        println!(
            "block {:?} {} {}",
            block_path.file_name().unwrap(),
            raw_count,
            txs.len()
        );

        // Parse state
        let accounts: HashMap<Address, EvmAccount> = serde_json::from_reader(BufReader::new(
            File::open(block_path.join("pre_state.json")).unwrap(),
        ))
        .unwrap();

        // Parse block hashes
        let block_hashes: BlockHashes = File::open(block_path.join("block_hashes.json"))
            .map(|file| {
                type SerializedFormat = HashMap<u64, B256, ahash::RandomState>;
                serde_json::from_reader::<_, SerializedFormat>(BufReader::new(file))
                    .unwrap()
                    .into()
            })
            .unwrap_or_default();

        mdbx::update_db_dir(&mdbx_dir, accounts.iter(), block_hashes.iter()).unwrap();

        handler(
            block,
            InMemoryStorage::new(accounts, Some(&bytecodes), block_hashes),
            &mdbx_dir,
        );
    }
}
