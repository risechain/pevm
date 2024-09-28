//! Fetch and snapshot a real block to disk for testing & benchmarking.
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::BufReader,
    num::NonZeroUsize,
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
// TODO: Test block after fetching it.
fn main() -> Result<(), Box<dyn Error>> {
    let Fetch { block_id, rpc_url } = Fetch::parse();

    // Define provider.
    let provider = ProviderBuilder::new().on_http(
        Url::parse(&rpc_url)
            .map_err(|err| format!("Invalid RPC URL supplied: {rpc_url}. {err}"))?,
    );

    // Retrive block from provider.
    let runtime = Runtime::new()?;
    let block = runtime
        .block_on(provider.get_block(block_id, BlockTransactionsKind::Full))
        .map_err(|err| format!("Failed to fetch block from provider. {err}"))?
        .ok_or(format!("No block found for ID: {:?}", block_id))?;

    // TODO: parameterize `chain` to add support for `OP`, `RISE`, and more.
    let chain = PevmEthereum::mainnet();
    let spec_id = chain.get_block_spec(&block.header).map_err(|err| {
        format!(
            "Failed to get block spec for block: {}. {:?}",
            block.header.number, err
        )
    })?;
    let storage = RpcStorage::new(provider, spec_id, BlockId::number(block.header.number - 1));

    // Execute the block and track the pre-state in the RPC storage.
    Pevm::default()
        .execute(&storage, &chain, block.clone(), NonZeroUsize::MIN, true)
        .map_err(|err| format!("Failed to execute block: {:?}", err))?;

    let block_dir = format!("data/blocks/{}", block.header.number);

    // Create block directory.
    fs::create_dir_all(block_dir.clone())
        .map_err(|err| format!("Failed to create block directory: {err}"))?;

    // Write block to disk.
    let block_file = File::create(format!("{block_dir}/block.json"))
        .map_err(|err| format!("Failed to create block file: {err}"))?;
    serde_json::to_writer(block_file, &block)
        .map_err(|err| format!("Failed to write block to file: {err}"))?;

    // Populate bytecodes and state from RPC storage.
    let mut state = BTreeMap::<Address, EvmAccount>::new();
    let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open("data/bytecodes.bincode") {
        Ok(file) => bincode::deserialize_from(BufReader::new(file))
            .map_err(|err| format!("Failed to deserialize bytecodes from file: {err}"))?,
        Err(_) => BTreeMap::new(),
    };
    bytecodes.extend(storage.get_cache_bytecodes());
    for (address, mut account) in storage.get_cache_accounts() {
        if let Some(code) = account.code.take() {
            let code_hash = account
                .code_hash
                .ok_or(format!("Failed to get code hash for: {}", address))?;
            assert_ne!(code_hash, KECCAK_EMPTY);
            bytecodes.insert(code_hash, code);
        }
        state.insert(address, account);
    }

    // Write bytecodes to disk.
    let file_bytecodes = File::create("data/bytecodes.bincode")
        .map_err(|err| format!("Failed to create bytecodes file: {err}"))?;
    bincode::serialize_into(file_bytecodes, &bytecodes)
        .map_err(|err| format!("Failed to write bytecodes to file: {err}"))?;

    // Write pre-state to disk.
    let file_state = File::create(format!("{block_dir}/pre_state.json"))
        .map_err(|err| format!("Failed to create pre-state file: {err}"))?;
    let json_state = serde_json::to_value(&state)
        .map_err(|err| format!("Failed to serialize pre-state to JSON: {err}"))?;
    serde_json::to_writer(file_state, &json_state)
        .map_err(|err| format!("Failed to write pre-state to file: {err}"))?;

    // Write block hashes to disk.
    let block_hashes: BTreeMap<u64, B256> = storage.get_cache_block_hashes().into_iter().collect();
    if !block_hashes.is_empty() {
        let file = File::create(format!("{block_dir}/block_hashes.json"))
            .map_err(|err| format!("Failed to create block hashes file: {err}"))?;
        serde_json::to_writer(file, &block_hashes)
            .map_err(|err| format!("Failed to write block hashes to file: {err}"))?;
    }

    Ok(())
}
