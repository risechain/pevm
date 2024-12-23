//! Fetch and snapshot a real block to disk for testing & benchmarking.
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufReader, Write},
    num::NonZeroUsize,
};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::{BlockId, BlockTransactionsKind};
use clap::Parser;
use color_eyre::eyre::{eyre, Result, WrapErr};
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use pevm::{
    chain::{PevmChain, PevmEthereum},
    EvmAccount, EvmCode, Pevm, RpcStorage,
};
use reqwest::Url;

#[derive(Parser, Debug)]
/// Fetch is a CLI tool to fetch a block from an RPC provider, and snapshot that block to disk.
struct Fetch {
    rpc_url: Url,
    block_id: BlockId,
}

// TODO: Binary formats to save disk?
// TODO: Test block after fetching it.
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let Fetch { block_id, rpc_url } = Fetch::parse();

    // Define provider.
    let provider = ProviderBuilder::new().on_http(rpc_url);

    // Retrive block from provider.
    let block = provider
        .get_block(block_id, BlockTransactionsKind::Full)
        .await
        .context("Failed to fetch block from provider")?
        .ok_or_else(|| eyre!("No block found for ID: {:?}", block_id))?;

    // TODO: parameterize `chain` to add support for `OP`, `RISE`, and more.
    let chain = PevmEthereum::mainnet();
    let spec_id = chain.get_block_spec(&block.header).with_context(|| {
        format!(
            "Failed to get block spec for block: {}",
            block.header.number
        )
    })?;
    let storage = RpcStorage::new(provider, spec_id, BlockId::number(block.header.number - 1));

    // Execute the block and track the pre-state in the RPC storage.
    Pevm::default()
        .execute(&storage, &chain, &block, NonZeroUsize::MIN, true)
        .context("Failed to execute block")?;

    let block_dir = format!("data/blocks/{}", block.header.number);

    // Create block directory.
    fs::create_dir_all(block_dir.clone()).context("Failed to create block directory")?;

    // Write block to disk.
    let block_file =
        File::create(format!("{block_dir}/block.json")).context("Failed to create block file")?;
    serde_json::to_writer(block_file, &block).context("Failed to write block to file")?;

    // Populate bytecodes and state from RPC storage.
    let mut state = BTreeMap::<Address, EvmAccount>::new();
    // TODO: Deduplicate logic with [for_each_block_from_disk] when there is more usage
    let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open("data/bytecodes.bincode.gz") {
        Ok(compressed_file) => {
            bincode::deserialize_from(GzDecoder::new(BufReader::new(compressed_file)))
                .context("Failed to deserialize bytecodes from file")?
        }
        Err(_) => BTreeMap::new(),
    };
    bytecodes.extend(storage.get_cache_bytecodes());
    for (address, mut account) in storage.get_cache_accounts() {
        if let Some(code) = account.code.take() {
            let code_hash = account
                .code_hash
                .ok_or_else(|| eyre!("Failed to get code hash for {address}"))?;
            assert_ne!(code_hash, KECCAK_EMPTY);
            bytecodes.insert(code_hash, code);
        }
        state.insert(address, account);
    }

    // Write compressed bytecodes to disk.
    let file_bytecodes = File::create("data/bytecodes.bincode.gz")
        .context("Failed to create compressed bytecodes file")?;
    let serialized_bytecodes =
        bincode::serialize(&bytecodes).context("Failed to serialize bytecodes to bincode")?;
    GzEncoder::new(file_bytecodes, Compression::default())
        .write_all(&serialized_bytecodes)
        .context("Failed to write bytecodes to file")?;

    // Write pre-state to disk.
    let file_state = File::create(format!("{block_dir}/pre_state.json"))
        .context("Failed to create pre-state file")?;
    let json_state =
        serde_json::to_value(&state).context("Failed to serialize pre-state to JSON")?;
    serde_json::to_writer(file_state, &json_state).context("Failed to write pre-state to file")?;

    // Write block hashes to disk.
    let block_hashes: BTreeMap<u64, B256> = storage.get_cache_block_hashes().into_iter().collect();
    if !block_hashes.is_empty() {
        let file = File::create(format!("{block_dir}/block_hashes.json"))
            .context("Failed to create block hashes file")?;
        serde_json::to_writer(file, &block_hashes)
            .context("Failed to write block hashes to file")?;
    }

    Ok(())
}
