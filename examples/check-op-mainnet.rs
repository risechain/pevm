#![allow(missing_docs)]

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockTransactionsKind};
use pevm::RpcStorage;
use reqwest::Url;
use revm::db::CacheDB;
use revm::primitives::SpecId;
use tokio::runtime::Runtime;

#[path = "../tests/common/mod.rs"]
pub mod common;

#[cfg(feature = "optimism")]
fn main() {
    let rpc_url = Url::parse("https://mainnet.optimism.io").unwrap();
    let block_number: u64 = 121252980;
    let runtime = Runtime::new().unwrap();
    let provider = ProviderBuilder::new().on_http(rpc_url.clone());
    let block = runtime
        .block_on(provider.get_block(BlockId::number(block_number), BlockTransactionsKind::Full))
        .unwrap()
        .unwrap();
    let spec_id = SpecId::ECOTONE;
    let rpc_storage = RpcStorage::new(provider, spec_id, BlockId::number(block_number - 1));
    let db = CacheDB::new(&rpc_storage);
    common::test_execute_alloy(
        pevm::ChainSpec::Optimism { chain_id: 10 },
        db.clone(),
        block.clone(),
        true,
    );
    // println!("{:?}", block.transactions);
}
