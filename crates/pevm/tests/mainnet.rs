//! Test with mainnet blocks

use pevm::chain::PevmEthereum;

pub mod common;

#[cfg(feature = "rpc-storage")]
fn get_rpc_url(env_var: &str, default_url: &str) -> reqwest::Url {
    std::env::var(env_var)
        .ok()
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| reqwest::Url::parse(default_url).unwrap())
}

#[cfg(feature = "rpc-storage")]
async fn run_block_tests<N, C>(
    provider: alloy_provider::RootProvider<alloy_transport_http::Http<reqwest::Client>, N>,
    chain: &C,
    block_numbers: &[u64],
) where
    N: alloy_provider::Network,
    C: pevm::chain::PevmChain + PartialEq + Send + Sync,
    N::BlockResponse: Into<alloy_rpc_types_eth::Block<C::Transaction>>,
{
    use alloy_provider::Provider;
    use alloy_rpc_types_eth::{BlockId, BlockTransactionsKind};

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
    use alloy_provider::ProviderBuilder;

    let rpc_url = get_rpc_url("ETHEREUM_RPC_URL", "https://eth-mainnet.public.blastapi.io");
    let provider = ProviderBuilder::new().on_http(rpc_url);
    let chain = PevmEthereum::mainnet();

    // First block under 50 transactions of each EVM-spec-changing fork
    run_block_tests(
        provider,
        &chain,
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

#[test]
fn mainnet_blocks_from_disk() {
    common::for_each_block_from_disk(|block, storage| {
        // Run several times to try catching a race condition if there is any.
        // 1000~2000 is a better choice for local testing after major changes.
        for _ in 0..3 {
            common::test_execute_alloy(&PevmEthereum::mainnet(), &storage, block.clone(), true)
        }
    });
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(all(feature = "rpc-storage", feature = "optimism"))]
async fn optimism_mainnet_blocks_from_rpc() {
    use alloy_provider::ProviderBuilder;
    use pevm::chain::PevmOptimism;

    let rpc_url = get_rpc_url("OPTIMISM_RPC_URL", "https://mainnet.optimism.io");
    let provider = ProviderBuilder::new()
        .network::<op_alloy_network::Optimism>()
        .on_http(rpc_url);
    let chain = PevmOptimism::mainnet();

    // First block under 50 transactions of each EVM-spec-changing fork
    run_block_tests(
        provider,
        &chain,
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
