// Test small blocks that we have specific handling for, like implicity fine-tuning
// the concurrency level, falling back to sequential processing, etc.

use alloy_primitives::{Address, U256};
use alloy_rpc_types::{Block, BlockTransactions, Transaction};
use revm::{
    primitives::{BlockEnv, SpecId, TransactTo, TxEnv},
    InMemoryDB,
};

pub mod common;

#[test]
fn empty_alloy_block() {
    common::test_execute_alloy(
        InMemoryDB::default(),
        Block {
            header: common::MOCK_ALLOY_BLOCK_HEADER.clone(),
            transactions: BlockTransactions::Full(Vec::new()),
            ..Block::default()
        },
        None,
    );
}

#[test]
fn empty_revm_block() {
    common::test_execute_revm(
        InMemoryDB::default(),
        SpecId::LATEST,
        BlockEnv::default(),
        Vec::new(),
    );
}

#[test]
fn one_tx_alloy_block() {
    common::test_execute_alloy(
        common::build_inmem_db(vec![common::mock_account(0)]),
        Block {
            // Legit header but with no transactions
            header: common::MOCK_ALLOY_BLOCK_HEADER.clone(),
            transactions: BlockTransactions::Full(vec![Transaction {
                from: Address::ZERO,
                to: Some(Address::ZERO),
                value: U256::from(1),
                gas_price: Some(1),
                gas: u64::MAX.into(),
                ..Transaction::default()
            }]),
            ..Block::default()
        },
        None,
    );
}

#[test]
fn one_tx_revm_block() {
    common::test_execute_revm(
        common::build_inmem_db(vec![common::mock_account(0)]),
        SpecId::LATEST,
        BlockEnv::default(),
        vec![TxEnv {
            caller: Address::ZERO,
            transact_to: TransactTo::Call(Address::ZERO),
            value: U256::from(1),
            gas_price: U256::from(1),
            ..TxEnv::default()
        }],
    );
}
