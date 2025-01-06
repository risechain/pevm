//! Test if block hashes are equal

#[tokio::test(flavor = "multi_thread")]
async fn block_hashes_equal() {
    use std::fs::File;
    use std::collections::BTreeMap;
    use alloy_primitives::B256;
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::{BlockNumberOrTag, BlockTransactionsKind};

    let block_hashes = match File::open("../../data/block_hashes.bincode") {
        Ok(compressed_file) => bincode::deserialize_from::<_, BTreeMap<u64, B256>>(compressed_file).unwrap(),
        Err(_) => BTreeMap::new(),
    };  
        
    let rpc_url = match std::env::var("ETHEREUM_RPC_URL") {
        // The empty check is for GitHub Actions where the variable is set with an empty string when unset!?
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => reqwest::Url::parse("https://eth.public-rpc.com").unwrap(),
    };

    let provider = ProviderBuilder::new().on_http(rpc_url.clone());

    for (block_number, hash) in block_hashes {
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number), BlockTransactionsKind::Hashes)
            .await
            .unwrap()
            .unwrap();

        let block_hash = block.header.hash;

        assert!(block_hash == hash);
    }

}