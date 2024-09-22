#![allow(missing_docs)]
use std::{collections::BTreeMap, fs::File, io::BufReader, num::NonZeroUsize, thread};

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
pub struct Fetch {
    /// The ID of the block.
    block_id: BlockId,
    /// The RPC url to connect to.
    rpc_url: String,
}

/// TODO: async main?
pub fn main() {
    let args = Fetch::parse();
    let Fetch { block_id, rpc_url } = args;

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

    // TODO: parameterize chain to later add support for `OP`, `RISE`.
    let chain = PevmEthereum::mainnet();
    let spec_id = chain.get_block_spec(&block.header).unwrap();
    let storage = RpcStorage::new(provider, spec_id, BlockId::number(block.header.number - 1));

    // Populate RPC storage.
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let mut pevm = Pevm::default();
    let _ = pevm
        .execute(&storage, &chain, block.clone(), concurrency_level, true)
        .unwrap();

    let dir = format!("data/blocks/{}", block.header.number);

    // Create blockfile.
    let block_file = File::create(format!("{dir}/block.json"))
        .unwrap_or_else(|e| panic!("Failed to create block file: {e}"));
    serde_json::to_writer(block_file, &block).expect("Failed to write block to file");

    // Load bytecodes.
    let mut state = BTreeMap::<Address, EvmAccount>::new();
    let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open("data/bytecodes.bincode") {
        Ok(file) => bincode::deserialize_from(BufReader::new(file)).unwrap(),
        Err(_) => BTreeMap::new(),
    };

    // Populate bytecodes and state from RPC storage.
    bytecodes.extend(storage.get_cache_bytecodes());
    for (address, mut account) in storage.get_cache_accounts() {
        if let Some(code) = account.code.take() {
            assert_ne!(account.code_hash.unwrap(), KECCAK_EMPTY);
            bytecodes.insert(account.code_hash.unwrap(), code);
        }
        state.insert(address, account);
    }

    // Write state and bytecodes to disk.
    let file_state = File::create(format!("{dir}/pre_state.json")).unwrap();
    let json_state = serde_json::to_value(&state).unwrap();
    serde_json::to_writer(file_state, &json_state).unwrap();
    let file_bytecodes = File::create("data/bytecodes.bincode").unwrap();
    bincode::serialize_into(file_bytecodes, &bytecodes).unwrap();

    // Write block hashes to disk.
    let block_hashes: BTreeMap<u64, B256> = storage.get_cache_block_hashes().into_iter().collect();
    if !block_hashes.is_empty() {
        let file = File::create(format!("{dir}/block_hashes.json")).unwrap();
        serde_json::to_writer(file, &block_hashes).unwrap();
    }
}
