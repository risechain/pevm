//! Fetch and snapshot a real block to disk for testing & benchmarking.
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufReader,
    num::NonZeroUsize,
    thread,
};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockTransactionsKind};
use clap::Parser;
use pevm::{
    chain::{PevmChain, PevmEthereum},
    EvmAccount, EvmCode, Pevm, RpcStorage,
};
use reqwest::Url;
use tokio::runtime::Runtime;

#[derive(Parser, Debug)]
/// Fetch is a CLI tool to fetch a block from an RPC provider, and snapshot that block to disk.
struct Fetch {
    rpc_url: String,
    block_id: BlockId,
}

// TODO: async main?
// TODO: Binary formats to save disk?
pub fn main() {
    let Fetch { block_id, rpc_url } = Fetch::parse();

    // Define provider.
    let provider = ProviderBuilder::new().on_http(
        Url::parse(&rpc_url).unwrap_or_else(|_| panic!("Invalid RPC URL supplied: {rpc_url}")),
    );

    // Retrive block from provider.
    let runtime = Runtime::new().unwrap();
    let block = runtime
        .block_on(provider.get_block(block_id, BlockTransactionsKind::Full))
        .expect("Failed to fetch block from provider")
        .unwrap_or_else(|| panic!("No block found for ID: {:?}", block_id));

    // TODO: parameterize `chain` to add support for `OP`, `RISE`, and more.
    let chain = PevmEthereum::mainnet();
    let spec_id = chain.get_block_spec(&block.header).unwrap_or_else(|e| {
        panic!(
            "Failed to get block spec for block: {}. {:?}",
            block.header.number, e
        )
    });
    let storage = RpcStorage::new(provider, spec_id, BlockId::number(block.header.number - 1));

    // Execute the block and track the pre-state in the RPC storage.
    let _ = Pevm::default()
        .execute(&storage, &chain, block.clone(), NonZeroUsize::MIN, true)
        .unwrap_or_else(|e| panic!("Failed to execute block: {:?}", e));

    let block_dir = format!("data/blocks/{}", block.header.number);

    // Create block directory.
    fs::create_dir_all(block_dir.clone())
        .unwrap_or_else(|e| panic!("Failed to create block directory: {e}"));

    // Create blockfile.
    let block_file = File::create(format!("{block_dir}/block.json"))
        .unwrap_or_else(|e| panic!("Failed to create block file: {e}"));
    serde_json::to_writer(block_file, &block)
        .unwrap_or_else(|e| panic!("Failed to write block to file: {e}"));

    // Populate bytecodes and state from RPC storage.
    let mut state = BTreeMap::<Address, EvmAccount>::new();
    let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open("data/bytecodes.bincode") {
        Ok(file) => bincode::deserialize_from(BufReader::new(file))
            .unwrap_or_else(|e| panic!("Failed to deserialize bytecodes from file: {e}")),
        Err(_) => BTreeMap::new(),
    };
    bytecodes.extend(storage.get_cache_bytecodes());
    for (address, mut account) in storage.get_cache_accounts() {
        if let Some(code) = account.code.take() {
            assert_ne!(account.code_hash.unwrap(), KECCAK_EMPTY);
            bytecodes.insert(account.code_hash.unwrap(), code);
        }
        state.insert(address, account);
    }

    // Write state and bytecodes to disk.
    let file_state = File::create(format!("{block_dir}/pre_state.json"))
        .unwrap_or_else(|e| panic!("Failed to create pre-state file: {e}"));
    let json_state = serde_json::to_value(&state).unwrap();
    serde_json::to_writer(file_state, &json_state)
        .unwrap_or_else(|e| panic!("Failed to write pre-state to file: {e}"));
    let file_bytecodes = File::create("data/bytecodes.bincode")
        .unwrap_or_else(|e| panic!("Failed to create bytecodes file: {e}"));
    bincode::serialize_into(file_bytecodes, &bytecodes)
        .unwrap_or_else(|e| panic!("Failed to write bytecodes to file: {e}"));

    // Write block hashes to disk.
    let block_hashes: BTreeMap<u64, B256> = storage.get_cache_block_hashes().into_iter().collect();
    if !block_hashes.is_empty() {
        let file = File::create(format!("{block_dir}/block_hashes.json"))
            .unwrap_or_else(|e| panic!("Failed to create block hashes file: {e}"));
        serde_json::to_writer(file, &block_hashes)
            .unwrap_or_else(|e| panic!("Failed to write block hashes to file: {e}"));
    }
}
