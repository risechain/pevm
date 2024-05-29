pub mod runner;
use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types::Header;
pub use runner::{
    build_inmem_db, execute_sequential, mock_account, test_execute_alloy, test_execute_revm,
};
pub mod storage;

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
