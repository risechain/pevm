#![allow(missing_docs)]

use alloy_primitives::Bloom;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockTransactionsKind};
use clap::Parser;
use common::assert_execution_result;
use op_alloy_network::Optimism;
use pevm::chain::PevmChain;
use pevm::RpcStorage;
use pevm::{Storage, StorageWrapper};
use reqwest::Url;
use revm::db::CacheDB;
use revm::primitives::SpecId;
use tokio::runtime::Runtime;

#[path = "../tests/common/mod.rs"]
pub mod common;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// JSON RPC URL
    #[arg(long, env, default_value = "https://mainnet.optimism.io")]
    rpc_url: String,

    /// Block number
    #[arg()]
    block_number: u64,
}

#[cfg(feature = "optimism")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use pevm::chain::PevmOptimism;

    let args = Args::parse();
    let rpc_url = Url::parse(&args.rpc_url)?;
    let block_number = args.block_number;
    let runtime = Runtime::new()?;
    let provider = ProviderBuilder::<_, _, Optimism>::default().on_http(rpc_url.clone());
    let block_opt = runtime
        .block_on(provider.get_block(BlockId::number(block_number), BlockTransactionsKind::Full))?;
    let block = block_opt.ok_or("missing block")?;
    let chain = PevmOptimism::mainnet();
    let spec_id = chain.get_block_spec(&block.header).unwrap();
    let pre_state_rpc_storage =
        RpcStorage::new(provider.clone(), spec_id, BlockId::number(block_number - 1));
    let pre_state_db = CacheDB::new(StorageWrapper(&pre_state_rpc_storage));

    let concurrency_level =
        std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN);

    let sequential_result = pevm::execute(
        &pre_state_db,
        &chain,
        block.clone(),
        concurrency_level,
        true,
    );
    let parallel_result = pevm::execute(
        &pre_state_db,
        &chain,
        block.clone(),
        concurrency_level,
        false,
    );

    assert_execution_result(&sequential_result, &parallel_result);
    let tx_results = sequential_result.unwrap();

    // We can only calculate the receipts root from Byzantium.
    // Before EIP-658 (https://eips.ethereum.org/EIPS/eip-658), the
    // receipt root is calculated with the post transaction state root,
    // which we doesn't have in these tests.
    if spec_id >= SpecId::BYZANTIUM {
        assert_eq!(
            block.header.receipts_root,
            chain.calculate_receipt_root(spec_id, &block.transactions, &tx_results)
        );
    }

    assert_eq!(
        block.header.logs_bloom,
        tx_results
            .iter()
            .map(|tx| tx.receipt.bloom_slow())
            .fold(Bloom::default(), |acc, bloom| acc.bit_or(bloom))
    );

    assert_eq!(
        block.header.gas_used,
        tx_results
            .iter()
            .last()
            .map(|result| result.receipt.cumulative_gas_used)
            .unwrap_or_default()
    );

    let observed_storage = pre_state_rpc_storage;
    for tx_result in tx_results {
        observed_storage.update_cache_accounts(tx_result.state);
    }
    let observed_accounts = observed_storage.get_cache_accounts();
    let expected_storage = RpcStorage::new(provider, spec_id, BlockId::number(block_number));

    for (address, account) in observed_accounts {
        let expected_basic = expected_storage
            .basic(&address)
            .unwrap()
            .unwrap_or_default();
        assert_eq!(expected_basic, account.basic);
        for (storage_key, storage_value) in account.storage {
            let expected_value = expected_storage.storage(&address, &storage_key).unwrap();
            assert_eq!(expected_value, storage_value);
        }
    }

    Ok(())
}
