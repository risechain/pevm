// Tests for the beneficiary account, especially for the lazy update of its balance to avoid
// "implicit" dependency among consecutive transactions.

use rand::random;
use revm::primitives::{
    alloy_primitives::U160, env::TxEnv, Address, BlockEnv, SpecId, TransactTo, U256,
};

pub mod common;

// Let's keep bumping this as execution performance improves,
// to ensure that the heavyweight cases are well tested and to
// not stack overflow, etc.
const BLOCK_SIZE: usize = 100_000;

fn test_beneficiary(get_address: fn(usize) -> Address) {
    common::test_execute_revm(
        // Mock the beneficiary account (`Address:ZERO`) and the next `BLOCK_SIZE` user accounts.
        common::build_inmem_db((0..=BLOCK_SIZE).map(common::mock_account)),
        SpecId::LATEST,
        BlockEnv::default(),
        // Mock `BLOCK_SIZE` transactions sending some tokens to itself.
        // Skipping `Address::ZERO` as the beneficiary account.
        (1..=BLOCK_SIZE)
            .map(|i| {
                // Randomly insert a beneficiary spending every ~256 txs
                let address = get_address(i);
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
        // Setting only the last tx as beneficiary for a
        // heavy evaluation/recursion all the way to the
        // top of the block.
        if i == BLOCK_SIZE {
            Address::from(U160::from(0))
        } else {
            Address::from(U160::from(i))
        }
    });
}
