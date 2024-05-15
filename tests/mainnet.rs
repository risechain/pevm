// TODO: Move this into `tests/ethereum`.
// TODO: `tokio::test`?

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::BlockId;
use block_stm_revm::RpcStorage;
use reqwest::Url;
use revm::db::CacheDB;
use tokio::runtime::Runtime;

pub mod common;

fn test_blocks(block_numbers: &[u64]) {
    // Minor but we can also turn this into a lazy static for reuse.
    let rpc_url: Url = std::env::var("RPC_URL")
        .unwrap_or("https://eth.llamarpc.com".to_string())
        .parse()
        .unwrap();
    let runtime = Runtime::new().unwrap();
    for block_number in block_numbers {
        let provider = ProviderBuilder::new().on_http(rpc_url.clone());
        let block = runtime
            .block_on(provider.get_block(BlockId::number(*block_number), true))
            .unwrap()
            .unwrap();
        let rpc_storage = RpcStorage::new(provider, BlockId::number(block_number - 1));
        let db = CacheDB::new(&rpc_storage);
        common::test_execute_alloy(db, block, None);
    }
}

#[test]
fn ethereum_mainnet_frontier_blocks() {
    test_blocks(&[
        46147,   // First block with a transaction
        1149999, // Last block
    ])
}
