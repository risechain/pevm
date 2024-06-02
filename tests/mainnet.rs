// TODO: Move this into `tests/ethereum`.
// TODO: `tokio::test`?

use std::fs::{self, File};

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::BlockId;
use pevm::RpcStorage;
use reqwest::Url;
use revm::db::CacheDB;
use tokio::runtime::Runtime;

pub mod common;

#[test]
fn mainnet_blocks_from_rpc() {
    let rpc_url: Url = std::env::var("RPC_URL")
        .unwrap_or("https://eth.llamarpc.com".to_string())
        .parse()
        .unwrap();

    // First block under 50 transactions of each EVM-spec-changing fork
    for block_number in [
        46147, // FRONTIER
        1150000, // HOMESTEAD
               // TODO: Enable these when CI is less flaky.
               // 2463002,  // TANGERINE
               // 2675000,  // SPURIOUS_DRAGON
               // 4370003,  // BYZANTIUM
               // 7280003,  // PETERSBURG
               // 9069001,  // ISTANBUL
               // 12244002, // BERLIN
               // 12965034, // LONDON
               // 15537395, // MERGE
               // 17035010, // SHANGHAI
               // 19426587, // CANCUN
    ] {
        let runtime = Runtime::new().unwrap();
        let provider = ProviderBuilder::new().on_http(rpc_url.clone());
        let block = runtime
            .block_on(provider.get_block(BlockId::number(block_number), true))
            .unwrap()
            .unwrap();
        let rpc_storage = RpcStorage::new(provider, BlockId::number(block_number - 1));
        let db = CacheDB::new(&rpc_storage);
        common::test_execute_alloy(db.clone(), block.clone(), None, true);

        // Snapshot blocks (for benchmark)
        // TODO: Port to a dedicated CLI instead?
        // TODO: Binary formats to save disk?
        if std::env::var("SNAPSHOT_BLOCKS") == Ok("1".to_string()) {
            let dir = format!("blocks/{block_number}");
            fs::create_dir_all(dir.clone()).unwrap();
            let file_block = File::create(format!("{dir}/block.json")).unwrap();
            serde_json::to_writer(file_block, &block).unwrap();
            let file_state = File::create(format!("{dir}/state_for_execution.json")).unwrap();
            serde_json::to_writer(file_state, &rpc_storage.get_cache()).unwrap();
        }
    }
}

#[test]
fn mainnet_blocks_from_disk() {
    common::for_each_block_from_disk(|block, state| {
        // Run several times to try catching a race condition if there is any.
        // 1000~2000 is a better choice for local testing after major changes.
        for _ in 0..3 {
            common::test_execute_alloy(
                common::build_in_mem(state.clone()),
                block.clone(),
                None,
                true,
            )
        }
    });
}
