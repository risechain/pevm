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
                *nonce += 1;
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
#[test]
fn debug_small_beneficiary() {
    use pevm::InMemoryStorage;
    use pevm::chain::PevmEthereum;
    use revm::{
        context::{TransactTo, TxEnv},
        primitives::{Address, U256, alloy_primitives::U160},
    };
    use std::collections::HashMap;

    let block_size = 3;
    let chain = PevmEthereum::mainnet();
    let mut nonces = HashMap::new();

    let storage = InMemoryStorage::new(
        (0..=block_size)
            .map(|i| {
                let address = Address::from(U160::from(i));
                let account = pevm::EvmAccount {
                    balance: U256::MAX.div_ceil(U256::from(2)),
                    nonce: 1,
                    ..Default::default()
                };
                (address, account)
            })
            .collect(),
        Default::default(),
        Default::default(),
    );

    // Last tx is beneficiary (addr 0) sending to self
    let txs: Vec<TxEnv> = (1..=block_size)
        .map(|i| {
            let address = if i == block_size {
                Address::from(U160::from(0))
            } else {
                Address::from(U160::from(i))
            };
            let nonce = nonces.entry(address).or_insert(0u64);
            *nonce += 1;
            TxEnv {
                caller: address,
                nonce: *nonce,
                kind: TransactTo::Call(address),
                value: U256::from(1),
                gas_price: 1,
                ..TxEnv::default()
            }
        })
        .collect();

    let seq = pevm::execute_revm_sequential(
        &chain,
        &storage,
        Default::default(),
        Default::default(),
        txs.clone(),
    )
    .unwrap();

    let par = pevm::Pevm::default()
        .execute_revm_parallel(
            &chain,
            &storage,
            Default::default(),
            Default::default(),
            txs,
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();

    for (i, (s, p)) in seq.iter().zip(par.iter()).enumerate() {
        println!("=== TX {} ===", i);
        let mut ss: Vec<_> = s.state.iter().collect();
        let mut ps: Vec<_> = p.state.iter().collect();
        ss.sort_by_key(|(a, _)| *a);
        ps.sort_by_key(|(a, _)| *a);

        println!("Sequential gas_used: {}", s.receipt.cumulative_gas_used);
        for (addr, acct) in &ss {
            println!(
                "  seq {}: {:?}",
                addr,
                acct.as_ref().map(|a| (a.balance, a.nonce))
            );
        }
        println!("Parallel gas_used: {}", p.receipt.cumulative_gas_used);
        for (addr, acct) in &ps {
            println!(
                "  par {}: {:?}",
                addr,
                acct.as_ref().map(|a| (a.balance, a.nonce))
            );
        }
        if s != p {
            println!("  *** MISMATCH ***");
        }
    }
}
