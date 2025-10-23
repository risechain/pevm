//! Test with mainnet blocks

pub mod common;

#[cfg(feature = "rpc-storage")]
async fn test_blocks_from_rpc<C>(url: reqwest::Url, chain: &C, block_numbers: &[u64])
where
    C: pevm::chain::PevmChain + PartialEq + Send + Sync,
    <C::Network as alloy_provider::Network>::BlockResponse:
        Into<alloy_rpc_types_eth::Block<C::Transaction>>,
{
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::{BlockId, BlockTransactionsKind};

    let provider = ProviderBuilder::new().network::<C::Network>().on_http(url);

    for &block_number in block_numbers {
        let block = provider
            .get_block(BlockId::number(block_number), BlockTransactionsKind::Full)
            .await
            .unwrap()
            .unwrap()
            .into();
        let spec_id = chain.get_block_spec(&block.header).unwrap();
        let rpc_storage =
            pevm::RpcStorage::new(provider.clone(), spec_id, BlockId::number(block_number - 1));
        common::test_execute_alloy(chain, &rpc_storage, block, true);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "rpc-storage")]
async fn mainnet_blocks_from_rpc() {
    let rpc_url = match std::env::var("ETHEREUM_RPC_URL") {
        // The empty check is for GitHub Actions where the variable is set with an empty string when unset!?
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => reqwest::Url::parse("https://eth-mainnet.public.blastapi.io").unwrap(),
    };

    // First block under 50 transactions of each EVM-spec-changing fork
    test_blocks_from_rpc(
        rpc_url,
        &pevm::chain::PevmEthereum::mainnet(),
        &[
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
        ],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(all(feature = "rpc-storage", feature = "optimism"))]
async fn optimism_mainnet_blocks_from_rpc() {
    let rpc_url = match std::env::var("OPTIMISM_RPC_URL") {
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => reqwest::Url::parse("https://mainnet.optimism.io").unwrap(),
    };

    // First block under 50 transactions of each EVM-spec-changing fork
    test_blocks_from_rpc(
        rpc_url,
        &pevm::chain::PevmOptimism::mainnet(),
        &[
            114874075, // CANYON (https://specs.optimism.io/protocol/canyon/overview.html)
                      // TODO: doesn't pass `Err(ExecutionError("Database(InvalidNonce(0))"))`
                      // 117874236, // ECOTONE (https://specs.optimism.io/protocol/ecotone/overview.html)
                      // 122874325, // FJORD (https://specs.optimism.io/protocol/fjord/overview.html)
                      // 125874340, // GRANITE (https://specs.optimism.io/protocol/granite/overview.html)
        ],
    )
    .await;
}

#[test]
fn mainnet_blocks_from_disk() {
    common::for_each_block_from_disk(|block, storage| {
        // Run several times to try catching a race condition if there is any.
        // 1000~2000 is a better choice for local testing after major changes.
        for _ in 0..3 {
            common::test_execute_alloy(
                &pevm::chain::PevmEthereum::mainnet(),
                &storage,
                block.clone(),
                true,
            )
        }
    });
}
