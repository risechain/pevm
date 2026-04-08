//! Test if snapshotted mainnet data is correct

use alloy_primitives::B256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::BlockNumberOrTag;
use std::collections::BTreeMap;
use std::fs::File;

#[tokio::test]
async fn snapshotted_mainnet_block_hashes() {
    let file = File::open("../../data/block_hashes.bincode").unwrap();
    let block_hashes = bincode::deserialize_from::<_, BTreeMap<u64, B256>>(file).unwrap();

    let rpc_url = match std::env::var("ETHEREUM_RPC_URL") {
        // The empty check is for GitHub Actions where the variable is set with an empty string when unset!?
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => reqwest::Url::parse("https://eth-mainnet.public.blastapi.io").unwrap(),
    };

    let provider = ProviderBuilder::new().connect_http(rpc_url);

    for (block_number, snapshotted_hash) in block_hashes {
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(snapshotted_hash, block.header.hash);
    }
}
