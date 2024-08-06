// Test small blocks that we have specific handling for, like implicit fine-tuning
// the concurrency level, falling back to sequential processing, etc.

use alloy_primitives::{Address, U256};
use alloy_rpc_types::{Block, BlockTransactions, Transaction};
use pevm::{chain::PevmEthereum, InMemoryStorage};
use revm::primitives::{TransactTo, TxEnv};

pub mod common;

#[test]
fn empty_alloy_block() {
    common::test_execute_alloy(
        &InMemoryStorage::default(),
        &PevmEthereum::mainnet(),
        Block {
            header: common::MOCK_ALLOY_BLOCK_HEADER.clone(),
            transactions: BlockTransactions::Full(Vec::new()),
            ..Block::default()
        },
        false,
    );
}

#[test]
fn empty_revm_block() {
    common::test_execute_revm(InMemoryStorage::default(), Vec::new());
}

#[test]
fn one_tx_alloy_block() {
    common::test_execute_alloy(
        &InMemoryStorage::new([common::mock_account(0)], None, []),
        &PevmEthereum::mainnet(),
        Block {
            // Legit header but with no transactions
            header: common::MOCK_ALLOY_BLOCK_HEADER.clone(),
            transactions: BlockTransactions::Full(vec![Transaction {
                transaction_type: Some(2),
                nonce: 1,
                from: Address::ZERO,
                to: Some(Address::ZERO),
                value: U256::from(1),
                max_fee_per_gas: Some(1),
                gas: u64::MAX.into(),
                ..Transaction::default()
            }]),
            ..Block::default()
        },
        false,
    );
}

#[test]
fn one_tx_revm_block() {
    common::test_execute_revm(
        InMemoryStorage::new([common::mock_account(0)], None, []),
        vec![TxEnv {
            caller: Address::ZERO,
            transact_to: TransactTo::Call(Address::ZERO),
            value: U256::from(1),
            gas_price: U256::from(1),
            ..TxEnv::default()
        }],
    );
}
