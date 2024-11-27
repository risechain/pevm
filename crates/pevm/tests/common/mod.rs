use std::{
    fs::{self, File},
    io::BufReader,
};

use alloy_consensus::{Signed, TxLegacy};
use alloy_primitives::{Address, Bytes, PrimitiveSignature, TxKind, B256, U256};
use alloy_rpc_types_eth::{Block, BlockTransactions, Header};
use flate2::bufread::GzDecoder;
use hashbrown::HashMap;
use pevm::{
    chain::PevmChain, BlockHashes, BuildSuffixHasher, Bytecodes, EvmAccount, InMemoryStorage,
};

/// runner module
pub mod runner;

/// runner module imports
pub use runner::{mock_account, test_execute_alloy, test_execute_revm};

/// storage module
pub mod storage;

/// The gas limit for a basic transfer transaction.
pub const RAW_TRANSFER_GAS_LIMIT: u64 = 21_000;

// TODO: Put somewhere better?
/// Iterates over blocks stored on disk and processes each block using the provided handler.
pub fn for_each_block_from_disk(mut handler: impl FnMut(Block, InMemoryStorage<'_>)) {
    let data_dir = std::path::PathBuf::from("../../data");

    // TODO: Deduplicate logic with [bin/fetch.rs] when there is more usage
    let bytecodes: Bytecodes = bincode::deserialize_from(GzDecoder::new(BufReader::new(
        File::open(data_dir.join("bytecodes.bincode.gz")).unwrap(),
    )))
    .unwrap();

    for block_path in fs::read_dir(data_dir.join("blocks")).unwrap() {
        let block_path = block_path.unwrap().path();
        let block_number = block_path.file_name().unwrap().to_str().unwrap();

        let block_dir = data_dir.join("blocks").join(block_number);

        // Parse block
        let block: Block = serde_json::from_reader(BufReader::new(
            File::open(block_dir.join("block.json")).unwrap(),
        ))
        .unwrap();

        // Parse state
        let accounts: HashMap<Address, EvmAccount, BuildSuffixHasher> = serde_json::from_reader(
            BufReader::new(File::open(block_dir.join("pre_state.json")).unwrap()),
        )
        .unwrap();

        // Parse block hashes
        let block_hashes: BlockHashes = File::open(block_dir.join("block_hashes.json"))
            .map(|file| serde_json::from_reader::<_, BlockHashes>(BufReader::new(file)).unwrap())
            .unwrap_or_default();

        handler(
            block,
            InMemoryStorage::new(accounts, Some(&bytecodes), block_hashes),
        );
    }
}

/// Test a chain with [`block_size`] independent raw transactions that transfer to itself
pub fn test_independent_raw_transfers<C>(chain: &C, block_size: usize)
where
    C: PevmChain + Send + Sync + PartialEq,
{
    let accounts: Vec<(Address, EvmAccount)> = (0..block_size).map(mock_account).collect();
    let block: Block<C::Transaction> = Block {
        header: Header {
            inner: alloy_consensus::Header {
                timestamp: 1710338135,
                excess_blob_gas: Some(0),
                gas_limit: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        },
        transactions: BlockTransactions::<C::Transaction>::Full(
            accounts
                .iter()
                .map(|(address, account)| {
                    chain.mock_tx(
                        Signed::new_unchecked(
                            TxLegacy {
                                chain_id: Some(chain.id()),
                                nonce: account.nonce,
                                gas_price: 0,
                                gas_limit: RAW_TRANSFER_GAS_LIMIT,
                                to: TxKind::Call(*address),
                                value: U256::from(1),
                                input: Bytes::default(),
                            },
                            PrimitiveSignature::new(U256::ZERO, U256::ZERO, false),
                            B256::default(),
                        )
                        .into(),
                        *address,
                    )
                })
                .collect(),
        ),
        ..Block::<C::Transaction>::default()
    };
    let storage = InMemoryStorage::new(accounts, None, []);
    test_execute_alloy(&storage, chain, block, false);
}
