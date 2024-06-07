use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
};

use ahash::AHashMap;
use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types::{Block, Header};
use pevm::InMemoryStorage;
use revm::{db::PlainAccount, primitives::KECCAK_EMPTY};

pub mod runner;
pub use runner::{assert_execution_result, mock_account, test_execute_alloy, test_execute_revm};
pub mod storage;

pub type ChainState = AHashMap<Address, PlainAccount>;
pub type BlockHashes = AHashMap<U256, B256>;

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
    total_difficulty: None,
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
    for block_path in fs::read_dir("blocks").unwrap() {
        let block_path = block_path.unwrap().path();
        let block_number = block_path.file_name().unwrap().to_str().unwrap();

        // Parse block
        let block: Block = serde_json::from_reader(BufReader::new(
            File::open(format!("blocks/{block_number}/block.json")).unwrap(),
        ))
        .unwrap();

        // Parse state
        let mut accounts: HashMap<Address, PlainAccount> = serde_json::from_reader(BufReader::new(
            File::open(format!("blocks/{block_number}/pre_state.json")).unwrap(),
        ))
        .unwrap();

        // Parse block hashes
        let block_hashes: BlockHashes =
            File::open(format!("blocks/{block_number}/block_hashes.json"))
                .map(|file| {
                    type T = HashMap<U256, B256, ahash::RandomState>;
                    serde_json::from_reader::<_, T>(BufReader::new(file))
                        .unwrap()
                        .into()
                })
                .unwrap_or_default();

        // Hacky but we don't serialize the whole account info to save space
        // So we need to resconstruct intermediate values upon deserializing.
        for (_, account) in accounts.iter_mut() {
            account.info.previous_or_original_balance = account.info.balance;
            account.info.previous_or_original_nonce = account.info.nonce;
            if let Some(code) = account.info.code.clone() {
                let code_hash = code.hash_slow();
                account.info.code_hash = code_hash;
                account.info.previous_or_original_code_hash = code_hash;
            } else {
                account.info.code_hash = KECCAK_EMPTY;
            }
        }
        handler(block, InMemoryStorage::new(accounts, block_hashes));
    }
}
