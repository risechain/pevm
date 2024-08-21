use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufReader,
};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockTransactionsKind};
use reqwest::Url;
use revm::db::CacheDB;
use tokio::runtime::Runtime;

use pevm::{
    chain::{PevmChain, PevmEthereum},
    EvmAccount, EvmCode, OnDiskStorage, RpcStorage, StorageWrapper,
};

pub mod common;

// TODO: [tokio::test]?
#[test]
fn mainnet_blocks_from_rpc() {
    let rpc_url = match std::env::var("RPC_URL") {
        // The empty check is for GitHub Actions where the variable is set with an empty string when unset!?
        Ok(value) if !value.is_empty() => value.parse().unwrap(),
        _ => Url::parse("https://eth.llamarpc.com").unwrap(),
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
        let db = CacheDB::new(&wrapped_storage);
        common::test_execute_alloy(&db, &chain, block.clone(), true);

        // Snapshot blocks (for benchmark)
        // TODO: Port to a dedicated CLI instead?
        // TODO: Binary formats to save disk?
        if std::env::var("SNAPSHOT_BLOCKS") == Ok("1".to_string()) {
            let dir = format!("data/blocks/{block_number}");
            fs::create_dir_all(dir.clone()).unwrap();
            let file_block = File::create(format!("{dir}/block.json")).unwrap();
            serde_json::to_writer(file_block, &block).unwrap();

            // TODO: Snapshot with consistent ordering for ease of diffing.
            // Currently [EvmStorage]'s storage ordering isn't consistent.
            let mut state = BTreeMap::<Address, EvmAccount>::new();
            let mut bytecodes: BTreeMap<B256, EvmCode> = match File::open("data/bytecodes.bincode")
            {
                Ok(file) => bincode::deserialize_from(BufReader::new(file)).unwrap(),
                Err(_) => BTreeMap::new(),
            };
            bytecodes.extend(rpc_storage.get_cache_bytecodes());
            for (address, mut account) in rpc_storage.get_cache_accounts() {
                if let Some(code) = account.code.take() {
                    assert_ne!(account.code_hash.unwrap(), KECCAK_EMPTY);
                    bytecodes.insert(account.code_hash.unwrap(), code);
                }
                state.insert(address, account);
            }

            let file_state = File::create(format!("{dir}/pre_state.json")).unwrap();
            serde_json::to_writer(file_state, &state).unwrap();
            let file_bytecodes = File::create("data/bytecodes.bincode").unwrap();
            bincode::serialize_into(file_bytecodes, &bytecodes).unwrap();

            // We convert to [BTreeMap] for consistent ordering & diffs between snapshots
            let block_hashes: BTreeMap<u64, B256> =
                rpc_storage.get_cache_block_hashes().into_iter().collect();
            if !block_hashes.is_empty() {
                let file = File::create(format!("{dir}/block_hashes.json")).unwrap();
                serde_json::to_writer(file, &block_hashes).unwrap();
            }
        }
    }
}

#[test]
fn mainnet_blocks_from_disk() {
    common::for_each_block_from_disk(|block, in_memory_storage, mdbx_dir| {
        // Run several times to try catching a race condition if there is any.
        // 1000~2000 is a better choice for local testing after major changes.
        for _ in 0..3 {
            common::test_execute_alloy(
                &in_memory_storage,
                &PevmEthereum::mainnet(),
                block.clone(),
                true,
            );
            let on_disk_storage = OnDiskStorage::open(mdbx_dir).unwrap();
            common::test_execute_alloy(
                &on_disk_storage,
                &PevmEthereum::mainnet(),
                block.clone(),
                true,
            );
        }
    });
}
