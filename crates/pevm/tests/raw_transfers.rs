//! Test raw transfers -- only send some ETH from one account to another without extra data.

use std::collections::HashMap;

use pevm::{InMemoryStorage, chain::PevmEthereum};
use rand::random;
use revm::{
    context::{TransactTo, TxEnv},
    primitives::{Address, U256, alloy_primitives::U160},
};

pub mod common;

#[test]
fn raw_transfers_independent() {
    let block_size = 100_000; // number of transactions
    let mut nonces = HashMap::new();

    common::test_execute_revm(
        &PevmEthereum::mainnet(),
        // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
        InMemoryStorage::new(
            (0..=block_size).map(common::mock_account).collect(),
            Default::default(),
            Default::default(),
        ),
        // Mock `block_size` transactions sending some tokens to itself.
        // Skipping `Address::ZERO` as the beneficiary account.
        (1..=block_size)
            .map(|i| {
                let address = Address::from(U160::from(i));
                let nonce = nonces.entry(address).or_insert(0);
                *nonce = *nonce + 1;
                TxEnv {
                    caller: address,
                    nonce: *nonce,
                    kind: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_limit: common::RAW_TRANSFER_GAS_LIMIT,
                    gas_price: 1,
                    ..TxEnv::default()
                }
            })
            .collect(),
    );
}

// The same sender sending multiple transfers with increasing nonces.
// These must be detected and executed in the correct order.
#[test]
fn raw_transfers_same_sender_multiple_txs() {
    let block_size = 5_000; // number of transactions

    let same_sender_address = Address::from(U160::from(1));
    let mut same_sender_nonce: u64 = 0;

    common::test_execute_revm(
        &PevmEthereum::mainnet(),
        // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
        InMemoryStorage::new(
            (0..=block_size).map(common::mock_account).collect(),
            Default::default(),
            Default::default(),
        ),
        (1..=block_size)
            .map(|i| {
                // Insert a "parallel" transaction every ~256 transactions
                // after the first ~30 guaranteed from the same sender.
                let (address, nonce) = if i > 30 && random::<u8>() == 0 {
                    (Address::from(U160::from(i)), 1)
                } else {
                    same_sender_nonce += 1;
                    (same_sender_address, same_sender_nonce)
                };
                TxEnv {
                    caller: address,
                    kind: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_limit: common::RAW_TRANSFER_GAS_LIMIT,
                    gas_price: 1,
                    nonce,
                    ..TxEnv::default()
                }
            })
            .collect(),
    );
}

#[test]
fn ethereum_empty_alloy_block() {
    common::test_independent_raw_transfers(&PevmEthereum::mainnet(), 0);
}

#[test]
fn ethereum_one_tx_alloy_block() {
    common::test_independent_raw_transfers(&PevmEthereum::mainnet(), 1);
}

#[test]
fn ethereum_independent_raw_transfers() {
    common::test_independent_raw_transfers(&PevmEthereum::mainnet(), 100_000);
}

#[test]
fn optimism_empty_alloy_block() {
    use pevm::chain::PevmOptimism;
    common::test_independent_raw_transfers(&PevmOptimism::mainnet(), 0);
}

#[test]
fn optimism_one_tx_alloy_block() {
    use pevm::chain::PevmOptimism;
    common::test_independent_raw_transfers(&PevmOptimism::mainnet(), 1);
}

#[test]
fn optimism_independent_raw_transfers() {
    use pevm::chain::PevmOptimism;
    common::test_independent_raw_transfers(&PevmOptimism::mainnet(), 100_000);
}
