use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
};

use ahash::AHashMap;
use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types::{Block, Header};
use pevm::{EvmAccount, EvmCode, InMemoryStorage};

pub mod runner;
pub use runner::{assert_execution_result, mock_account, test_execute_alloy, test_execute_revm};
pub mod storage;

pub type ChainState = AHashMap<Address, EvmAccount>;
pub type Bytecodes = AHashMap<B256, EvmCode>;
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

// TODO: Put somewhere better?
pub fn for_each_block_from_disk(mut handler: impl FnMut(Block, InMemoryStorage)) {
    // Parse bytecodes
    let bytecodes: AHashMap<B256, EvmCode> = bincode::deserialize_from(BufReader::new(
        File::open("data/bytecodes.bincode").unwrap(),
    ))
    .unwrap();

    for block_path in fs::read_dir("data/blocks").unwrap() {
        let block_path = block_path.unwrap().path();
        let block_number = block_path.file_name().unwrap().to_str().unwrap();

        // Parse block
        let block: Block = serde_json::from_reader(BufReader::new(
            File::open(format!("data/blocks/{block_number}/block.json")).unwrap(),
        ))
        .unwrap();

        // Parse state
        let accounts: HashMap<Address, EvmAccount> = serde_json::from_reader(BufReader::new(
            File::open(format!("data/blocks/{block_number}/pre_state.json")).unwrap(),
        ))
        .unwrap();

        // Parse block hashes
        let block_hashes: BlockHashes =
            File::open(format!("data/blocks/{block_number}/block_hashes.json"))
                .map(|file| {
                    type SerializedFormat = HashMap<u64, B256, ahash::RandomState>;
                    serde_json::from_reader::<_, SerializedFormat>(BufReader::new(file))
                        .unwrap()
                        .into()
                })
                .unwrap_or_default();

        handler(
            block,
            InMemoryStorage::new(accounts, Some(&bytecodes), block_hashes),
        );
    }
}
