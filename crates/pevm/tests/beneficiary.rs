//! Tests for the beneficiary account, especially for the lazy update of its balance to avoid
//! "implicit" dependency among consecutive transactions.

use std::collections::HashMap;

use common::MockEvmTx;
use pevm::InMemoryStorage;
use pevm::chain::{PevmEthereum, PevmRise};
use rand::random;
use revm::{
    context::{TransactTo, TxEnv},
    primitives::{Address, U256, alloy_primitives::U160},
};

pub mod common;

const BLOCK_SIZE: usize = 100_000;

fn test_beneficiary<C>(chain: &C, get_address: fn(usize) -> Address)
where
    C: MockEvmTx + PartialEq + Send + Sync,
{
    let mut nonces = HashMap::new();
    common::test_execute_revm(
        chain,
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
                *nonce += 1;
                chain.mock_evm_tx(TxEnv {
                    caller: address,
                    nonce: *nonce,
                    kind: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_price: 1,
                    ..TxEnv::default()
                })
            })
            .collect(),
    );
}

// Randomly insert a beneficiary spending every ~256 txs
fn random_beneficiary_address(i: usize) -> Address {
    if random::<u8>() == 0 {
        Address::from(U160::from(0))
    } else {
        Address::from(U160::from(i))
    }
}

// Setting only the last tx as beneficiary for a heavy
// evaluation all the way to the top of the block.
fn last_beneficiary_address(i: usize) -> Address {
    if i == BLOCK_SIZE {
        Address::from(U160::from(0))
    } else {
        Address::from(U160::from(i))
    }
}

#[test]
fn ethereum_beneficiary_random() {
    test_beneficiary(&PevmEthereum::mainnet(), random_beneficiary_address);
}

#[test]
fn rise_beneficiary_random() {
    test_beneficiary(&PevmRise, random_beneficiary_address);
}

#[test]
fn ethereum_beneficiary_heavy_evaluation() {
    test_beneficiary(&PevmEthereum::mainnet(), last_beneficiary_address);
}

#[test]
fn rise_beneficiary_heavy_evaluation() {
    test_beneficiary(&PevmRise, last_beneficiary_address);
}
