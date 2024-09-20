use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
};

use alloy_primitives::{uint, Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types::{Block, BlockTransactions, Header, Signature};
use pevm::{chain::PevmChain, BlockHashes, Bytecodes, EvmAccount, InMemoryStorage};

pub mod runner;
pub use runner::{mock_account, test_execute_alloy, test_execute_revm};
pub mod storage;

pub static MOCK_ALLOY_BLOCK_HEADER: Header = Header {
    // Minimal requirements for execution
    number: 1,
    timestamp: 1710338135,
    mix_hash: Some(B256::ZERO),
    excess_blob_gas: Some(0),
    gas_limit: u128::MAX,
    // Defaults
    hash: B256::ZERO,
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

const MOCK_SIGNATURE: Signature = Signature {
    r: uint!(1_U256),
    s: uint!(1_U256),
    v: U256::ZERO,
    y_parity: None,
};

pub const RAW_TRANSFER_GAS_LIMIT: u64 = 21_000;

// TODO: Put somewhere better?
pub fn for_each_block_from_disk(mut handler: impl FnMut(Block, InMemoryStorage)) {
    // Parse bytecodes
    let bytecodes: Bytecodes = bincode::deserialize_from(BufReader::new(
        File::open("data/bytecodes.bincode").unwrap(),
    ))
    .unwrap();

    // Write bytecodes to disk as json JSON is same.
    // let file_bytecodes = File::create("data/bytecodesOld.json").unwrap();
    // serde_json::to_writer(file_bytecodes, &bytecodes).unwrap();

    // println!("Bytecodes: {:#?}", bytecodes);

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
                    serde_json::from_reader::<_, BlockHashes>(BufReader::new(file)).unwrap()
                })
                .unwrap_or_default();

        handler(
            block,
            InMemoryStorage::new(accounts, Some(&bytecodes), block_hashes),
        );
    }
}

// Test a chain with [block_size] independent raw transactions that transfer to itself
pub fn test_independent_raw_transfers<C>(chain: &C, block_size: usize)
where
    C: PevmChain + Send + Sync + PartialEq,
    C::Transaction: Default,
{
    let accounts: Vec<(Address, EvmAccount)> = (0..block_size).map(mock_account).collect();
    let block: Block<C::Transaction> = Block {
        header: MOCK_ALLOY_BLOCK_HEADER.clone(),
        transactions: BlockTransactions::<C::Transaction>::Full(
            accounts
                .iter()
                .map(|(address, account)| {
                    chain.build_tx_from_alloy_tx(alloy_rpc_types::Transaction {
                        chain_id: Some(chain.id()),
                        transaction_type: Some(2),
                        from: *address,
                        to: Some(*address),
                        value: U256::from(1),
                        gas: RAW_TRANSFER_GAS_LIMIT.into(),
                        max_fee_per_gas: Some(1),
                        max_priority_fee_per_gas: Some(0),
                        nonce: account.nonce,
                        signature: Some(MOCK_SIGNATURE),
                        ..alloy_rpc_types::Transaction::default()
                    })
                })
                .collect(),
        ),
        ..Block::<C::Transaction>::default()
    };
    let storage = InMemoryStorage::new(accounts, None, []);
    test_execute_alloy(&storage, chain, block, false);
}
