//! Tests for the beneficiary account, especially for the lazy update of its balance to avoid
//! "implicit" dependency among consecutive transactions.

use std::collections::HashMap;

use pevm::InMemoryStorage;
use pevm::chain::PevmEthereum;
use rand::random;
use revm::{
    context::{TransactTo, TxEnv},
    primitives::{Address, U256, alloy_primitives::U160},
};

pub mod common;

const BLOCK_SIZE: usize = 100_000;

fn test_beneficiary(get_address: fn(usize) -> Address) {
    let mut nonces = HashMap::new();
    common::test_execute_revm(
        &PevmEthereum::mainnet(),
        // Mock the beneficiary account (`Address:ZERO`) and the next `BLOCK_SIZE` user accounts.
        InMemoryStorage::new(
            (0..=BLOCK_SIZE).map(common::mock_account).collect(),
            Default::default(),
            Default::default(),
        ),
        // Mock `BLOCK_SIZE` transactions sending some tokens to itself.
        // Skipping `Address::ZERO` as the beneficiary account.
        (1..=BLOCK_SIZE)
            .map(|i| {
                // Randomly insert a beneficiary spending every ~256 txs
                let address = get_address(i);
                let nonce = nonces.entry(address).or_insert(0);
                *nonce = *nonce + 1;
                TxEnv {
                    caller: address,
                    nonce: *nonce,
                    kind: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_price: 1,
                    ..TxEnv::default()
                }
            })
            .collect(),
    );
}

#[test]
fn beneficiary_random() {
    test_beneficiary(|i| {
        // Randomly insert a beneficiary spending every ~256 txs
        if random::<u8>() == 0 {
            Address::from(U160::from(0))
        } else {
            Address::from(U160::from(i))
        }
    });
}

#[test]
fn beneficiary_heavy_evaluation() {
    test_beneficiary(|i| {
        // Setting only the last tx as beneficiary for a heavy
        // evaluation all the way to the top of the block.
        if i == BLOCK_SIZE {
            Address::from(U160::from(0))
        } else {
            Address::from(U160::from(i))
        }
    });
}
