// Test raw transfers -- only send some ETH from one account to another without extra data.

use rand::random;
use revm::primitives::{
    alloy_primitives::U160, env::TxEnv, Account, Address, BlockEnv, SpecId, TransactTo, U256,
};

pub mod common;

#[test]
fn raw_transfers_independent() {
    let block_size = 100_000; // number of transactions

    // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
    let accounts: Vec<(Address, Account)> = (0..=block_size).map(common::mock_account).collect();

    common::test_txs(
        &accounts,
        SpecId::LATEST,
        BlockEnv::default(),
        // Mock `block_size` transactions sending some tokens to itself.
        // Skipping `Address::ZERO` as the beneficiary account.
        (1..=block_size)
            .map(|i| {
                let address = Address::from(U160::from(i));
                TxEnv {
                    caller: address,
                    transact_to: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_price: U256::from(1),
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

    // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
    let accounts: Vec<(Address, Account)> = (0..=block_size).map(common::mock_account).collect();

    let same_sender_address = Address::from(U160::from(1));
    let mut same_sender_nonce: u64 = 0;

    common::test_txs(
        &accounts,
        SpecId::LATEST,
        BlockEnv::default(),
        (1..=block_size)
            .map(|i| {
                // Insert a "parallel" transaction every ~256 transactions
                // after the first ~30 guaranteed from the same sender.
                let (address, nonce) = if i > 30 && random::<u8>() == 0 {
                    (Address::from(U160::from(i)), 0)
                } else {
                    let nonce = same_sender_nonce;
                    same_sender_nonce += 1;
                    (same_sender_address, nonce)
                };
                TxEnv {
                    caller: address,
                    transact_to: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_price: U256::from(1),
                    nonce: Some(nonce),
                    ..TxEnv::default()
                }
            })
            .collect(),
    );
}
