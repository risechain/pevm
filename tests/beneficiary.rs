// Tests for the beneficiary account, especially for the lazy update of its balance to avoid
// "implicit" dependency among consecutive transactions.
// Currently, we randomly insert a beneficiary spending in the middle of the block.
// TODO: Add more test scenarios around the beneficiary account's activities in the block.

use rand::random;
use revm::primitives::{
    alloy_primitives::U160, env::TxEnv, Account, Address, BlockEnv, SpecId, TransactTo, U256,
};

mod common;

#[test]
fn beneficiary() {
    let spec_id = SpecId::LATEST;
    let block_env = BlockEnv::default();
    let block_size = 100_000; // number of transactions

    // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
    let accounts: Vec<(Address, Account)> = (0..=block_size).map(common::mock_account).collect();

    common::test_txs(
        &accounts,
        spec_id,
        block_env,
        // Mock `block_size` transactions sending some tokens to itself.
        // Skipping `Address::ZERO` as the beneficiary account.
        (1..=block_size)
            .map(|i| {
                // Randomly insert a beneficiary spending every ~256 txs
                let address = if random::<u8>() == 0 {
                    Address::from(U160::from(0))
                } else {
                    Address::from(U160::from(i))
                };
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
