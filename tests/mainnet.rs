use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockTransactionsKind};
use reqwest::Url;
use tokio::runtime::Runtime;

use pevm::{
    chain::{PevmChain, PevmEthereum},
    RpcStorage, StorageWrapper,
};

pub mod common;

// TODO: [tokio::test]?
#[test]
fn mainnet_blocks_from_rpc() {
    let rpc_url = match std::env::var("ETHEREUM_RPC_URL") {
        // The empty check is for GitHub Actions where the variable is set with an empty string when unset!?
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => Url::parse("https://eth.public-rpc.com").unwrap(),
    };

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
            .block_on(
                provider.get_block(BlockId::number(block_number), BlockTransactionsKind::Full),
            )
            .unwrap()
            .unwrap();
        let chain = PevmEthereum::mainnet();
        let spec_id = chain.get_block_spec(&block.header).unwrap();
        let rpc_storage = RpcStorage::new(provider, spec_id, BlockId::number(block_number - 1));
        let wrapped_storage = StorageWrapper(&rpc_storage);
        common::test_execute_alloy(&wrapped_storage, &chain, block, true);
    }
}

#[test]
fn mainnet_blocks_from_disk() {
    common::for_each_block_from_disk(|block, storage| {
        let db = StorageWrapper(&storage);
        // Run several times to try catching a race condition if there is any.
        // 1000~2000 is a better choice for local testing after major changes.
        for _ in 0..3 {
            common::test_execute_alloy(&db, &PevmEthereum::mainnet(), block.clone(), true)
        }
    });
}
