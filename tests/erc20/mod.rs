pub mod contract;

use ahash::AHashMap;
use contract::ERC20Token;
use pevm::{Bytecodes, EvmAccount};
use revm::primitives::{uint, Address, TransactTo, TxEnv, U256};

use crate::common::ChainState;

pub const GAS_LIMIT: u64 = 26_938;

// TODO: Better randomness control. Sometimes we want duplicates to test
// dependent transactions, sometimes we want to guarantee non-duplicates
// for independent benchmarks.
fn generate_addresses(length: usize) -> Vec<Address> {
    (0..length).map(|_| Address::new(rand::random())).collect()
}

pub fn generate_cluster(
    num_families: usize,
    num_people_per_family: usize,
    num_transfers_per_person: usize,
) -> (ChainState, Bytecodes, Vec<TxEnv>) {
    let families: Vec<Vec<Address>> = (0..num_families)
        .map(|_| generate_addresses(num_people_per_family))
        .collect();

    let people_addresses: Vec<Address> = families.clone().into_iter().flatten().collect();

    let gld_address = Address::new(rand::random());

    let gld_account = ERC20Token::new("Gold Token", "GLD", 18, 222_222_000_000_000_000_000_000u128)
        .add_balances(&people_addresses, uint!(1_000_000_000_000_000_000_U256))
        .build();

    let mut state = AHashMap::from([(gld_address, gld_account)]);
    let mut txs = Vec::new();

    for person in people_addresses.iter() {
        state.insert(
            *person,
            EvmAccount {
                balance: uint!(4_567_000_000_000_000_000_000_U256),
                ..EvmAccount::default()
            },
        );
    }

    for nonce in 0..num_transfers_per_person {
        for family in families.iter() {
            for person in family {
                let recipient = family[(rand::random::<usize>()) % (family.len())];
                let calldata = ERC20Token::transfer(recipient, U256::from(rand::random::<u8>()));

                txs.push(TxEnv {
                    caller: *person,
                    gas_limit: GAS_LIMIT,
                    gas_price: U256::from(0xb2d05e07u64),
                    transact_to: TransactTo::Call(gld_address),
                    data: calldata,
                    nonce: Some(nonce as u64),
                    ..TxEnv::default()
                })
            }
        }
    }

    let mut bytecodes = Bytecodes::new();
    for account in state.values_mut() {
        let code = account.code.take();
        if let Some(code) = code {
            bytecodes.insert(account.code_hash.unwrap(), code);
        }
    }

    (state, bytecodes, txs)
}
