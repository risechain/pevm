// TODO: Move this into `tests/ethereum`.
// TODO: `tokio::test`?

use std::fs::{self, File};

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
        common::test_execute_alloy(db, block.clone(), None);

        // Snapshot blocks (for benchmark)
        // TODO: Binary formats to save disk?
        // TODO: Put behind a feature flag?
        let dir = format!("blocks/{block_number}");
        fs::create_dir_all(dir.clone()).unwrap();
        let file_block = File::create(format!("{dir}/block.json")).unwrap();
        serde_json::to_writer(file_block, &block).unwrap();
        let file_state = File::create(format!("{dir}/state_for_execution.json")).unwrap();
        serde_json::to_writer(file_state, &rpc_storage.get_cache()).unwrap();
    }
}

// Grouping together to avoid running in parallel
// that would trigger rate-limiting in CI.
// TODO: Use a feature flag to disable large tests
// on CI instead of commenting them out.
#[test]
fn ethereum_mainnet_blocks() {
    test_blocks(&[
        // FRONTIER
        46147, // First block with a transaction
              // 930196,  // Relatively large block
              // 1149999, // Last block
              // HOMESTEAD
              // 1150000, // First block
              // 2179522, // Relatively large block
              // 2462997, // Last block with a transaction
              // TANGERINE
              // 2463002, // First block with a transaction
              // 2641321, // Relatively large block
              // 2674998, // Last block with a transaction
              // SPURIOUS_DRAGON
              // 2675000, // First block
              // 4330482, // Relatively large block
              // 4369999, // Last block
              // BYZANTIUM
              // 4370000, // First block
              // 5891667, // Relatively large block
              // 7279999, // Last block
              // PETERSBURG
              // 7280000, // First block
              // 8889776, // Relatively large block
              // 9068998, // Last block with a transaction
              // ISTANBUL
              // 9069000, // First block
              // 11814555, // Relatively large block
              // 12243999, // Last block
              // BERLIN
              // 12244000, // First block
              // 12520364, // Relatively large block
              // 12964999, // Last block
              // LONDON
              // 12965000, // First block
              // 13217637, // Relatively large block
              // 15537393, // Last block
              // MERGE
              // 15537394, // First block
              // 16146267, // Relatively large block
              // 17034869, // Last block
              // SHANGHAI
              // 17034870, // First block
              // 17666333, // Relatively large block
              // 19426586, // Last block
              // CANCUN
              // 19426587, // First block
              // 19638737, // Relatively large block
    ])
}
