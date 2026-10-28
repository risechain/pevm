//! Test small blocks that we have specific handling for, like implicit fine-tuning
//! the concurrency level, falling back to sequential processing, etc.

use alloy_primitives::{Address, U256};
use common::MockEvmTx;
use pevm::InMemoryStorage;
use pevm::chain::{PevmEthereum, PevmRise};
use revm::context::{TransactTo, TxEnv};

pub mod common;

fn test_empty_revm_block<C>(chain: &C)
where
    C: MockEvmTx + PartialEq + Send + Sync,
{
    common::test_execute_revm(chain, InMemoryStorage::default(), Vec::new());
}

#[test]
fn ethereum_empty_revm_block() {
    test_empty_revm_block(&PevmEthereum::mainnet());
}

#[test]
fn rise_empty_revm_block() {
    test_empty_revm_block(&PevmRise);
}

fn test_one_tx_revm_block<C>(chain: &C)
where
    C: MockEvmTx + PartialEq + Send + Sync,
{
    common::test_execute_revm(
        chain,
        InMemoryStorage::new(
            [common::mock_account(0)].into_iter().collect(),
            Default::default(),
            Default::default(),
        ),
        vec![chain.mock_evm_tx(TxEnv {
            caller: Address::ZERO,
            nonce: 1,
            kind: TransactTo::Call(Address::ZERO),
            value: U256::from(1),
            gas_price: 1,
            ..TxEnv::default()
        })],
    );
}

#[test]
fn ethereum_one_tx_revm_block() {
    test_one_tx_revm_block(&PevmEthereum::mainnet());
}

#[test]
fn rise_one_tx_revm_block() {
    test_one_tx_revm_block(&PevmRise);
}
