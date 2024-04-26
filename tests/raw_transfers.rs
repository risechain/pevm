// Test raw transfers -- only send some ETH from one account to another without extra data.
// Currently, we only have a no-state-conflict test of user account sending to themselves.
// TODO: Add more tests of accounts cross-transferring to create state depdendencies.

use revm::primitives::{alloy_primitives::U160, env::TxEnv, Address, BlockEnv, TransactTo, U256};

mod common;

#[test]
fn raw_transfers() {
    let block_size = 100_000; // number of transactions
    let block_env = BlockEnv::default();

    common::test_txs(
        block_env,
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
