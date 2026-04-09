//! Fetch and snapshot a real block to disk for testing & benchmarking.
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufReader,
    num::NonZeroUsize,
};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, ProviderBuilder, RootProvider, network::Ethereum};
use alloy_rpc_types_eth::BlockId;
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr, eyre};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use op_alloy_network::Optimism;
use pevm::{
    EvmAccount, EvmCode, Pevm, RpcStorage,
    chain::{PevmChain, PevmEthereum, PevmRise},
};
use reqwest::Url;
use serde::Serialize;

#[derive(clap::ValueEnum, Debug, Clone)]
enum ChainChoice {
    Ethereum,
    Rise,
}

#[derive(Parser, Debug)]
/// Fetch is a CLI tool to fetch a block from an RPC provider, and snapshot that block to disk.
struct Fetch {
    #[arg(long, value_enum, default_value = "ethereum")]
    chain: ChainChoice,
    rpc_url: Url,
    block_id: BlockId,
}

/// Fetch a block and snapshot it to `{data_dir}/blocks/{block_number}/`.
/// Bytecodes and block hashes are accumulated in `{data_dir}/`.
async fn run<C>(
    chain: C,
    provider: RootProvider<C::Network>,
    block_id: BlockId,
    data_dir: &str,
) -> Result<()>
where
    C: PevmChain + Send + Sync,
    C::Transaction: Serialize,
{
    // Retrieve block from provider.
    let block = provider
        .get_block(block_id)
        .full()
        .await
        .context("Failed to fetch block from provider")?
        .ok_or_else(|| eyre!("No block found for ID: {block_id:?}"))?
        .into();

    let spec_id = chain
        .get_block_spec(&block.header)
        .map_err(|e| {
            eyre!(
                "Failed to get block spec for block {}: {e}",
                block.header.number
            )
        })?
        .into();

    let storage = RpcStorage::new(provider, spec_id, BlockId::number(block.header.number - 1));

    // Execute the block and track the pre-state in the RPC storage.
    Pevm::default()
        .execute(&chain, &storage, &block, NonZeroUsize::MIN, true)
        .map_err(|e| eyre!("Failed to execute block: {e:?}"))?;

    let block_dir = format!("{data_dir}/blocks/{}", block.header.number);

    // Create block directory.
    fs::create_dir_all(&block_dir).context("Failed to create block directory")?;

    // Write block to disk.
    let block_file =
        File::create(format!("{block_dir}/block.json")).context("Failed to create block file")?;
    serde_json::to_writer(block_file, &block).context("Failed to write block to file")?;

    // Populate bytecodes and state from RPC storage.
    // TODO: Deduplicate logic with [for_each_block_from_disk] when there is more usage
    let bytecodes_path = format!("{data_dir}/bytecodes.bincode.gz");
    let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open(&bytecodes_path) {
        Ok(compressed_file) => bincode::serde::decode_from_std_read(
            &mut GzDecoder::new(BufReader::new(compressed_file)),
            bincode::config::standard(),
        )
        .context("Failed to deserialize bytecodes from file")?,
        Err(_) => BTreeMap::new(),
    };
    let (chainstate, cached_bytecodes, cached_block_hashes) = storage.into_snapshot();
    bytecodes.extend(cached_bytecodes);

    let mut state = BTreeMap::<Address, EvmAccount>::new();
    for (address, mut account) in chainstate {
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
    let mut writer_bytecodes = File::create(&bytecodes_path)
        .map(|f| GzEncoder::new(f, Compression::default()))
        .context("Failed to create compressed bytecodes file")?;
    bincode::serde::encode_into_std_write(
        &bytecodes,
        &mut writer_bytecodes,
        bincode::config::standard(),
    )
    .context("Failed to write bytecodes to file")?;

    // Write pre-state to disk.
    let file_state = File::create(format!("{block_dir}/pre_state.json"))
        .context("Failed to create pre-state file")?;
    serde_json::to_writer(file_state, &state).context("Failed to write pre-state to file")?;

    // TODO: Deduplicate logic with [for_each_block_from_disk] when there is more usage
    let block_hashes_path = format!("{data_dir}/block_hashes.bincode");
    let mut block_hashes = match File::open(&block_hashes_path) {
        Ok(mut file) => bincode::serde::decode_from_std_read::<BTreeMap<u64, B256>, _, _>(
            &mut file,
            bincode::config::standard(),
        )
        .context("Failed to deserialize block hashes from file")?,
        Err(_) => BTreeMap::new(),
    };
    block_hashes.extend(cached_block_hashes);

    if !block_hashes.is_empty() {
        let mut file =
            File::create(&block_hashes_path).context("Failed to create block hashes file")?;
        bincode::serde::encode_into_std_write(
            &block_hashes,
            &mut file,
            bincode::config::standard(),
        )
        .context("Failed to write block hashes to file")?;
    }

    println!("Fetched block {}.", block.header.number,);

    Ok(())
}

// TODO: Test block after fetching it.
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let Fetch {
        block_id,
        rpc_url,
        chain,
    } = Fetch::parse();

    match chain {
        ChainChoice::Ethereum => {
            let provider = ProviderBuilder::<_, _, Ethereum>::default().connect_http(rpc_url);
            run(PevmEthereum::mainnet(), provider, block_id, "data/ethereum").await
        }
        ChainChoice::Rise => {
            let provider = ProviderBuilder::<_, _, Optimism>::default().connect_http(rpc_url);
            run(PevmRise, provider, block_id, "data/rise").await
        }
    }
}
