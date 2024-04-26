// Test raw transfers -- only send some ETH from one account to another without extra data.
// Currently, we only have a no-state-conflict test of user account sending to themselves.
// TODO: Add more tests of accounts cross-transferring to create state depdendencies.

use revm::primitives::{
    alloy_primitives::U160, env::TxEnv, Address, BlockEnv, SpecId, TransactTo, U256,
};

mod common;

#[test]
fn raw_transfers() {
    let spec_id = SpecId::LATEST;
    let block_env = BlockEnv::default();
    let block_size = 100_000; // number of transactions

    common::test_txs(
        spec_id,
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
