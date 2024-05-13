// Each cluster has one ERC20 contract and X families.
// Each family has Y people.
// Each person performs Z transfers to random people within the family.

pub mod common;

use crate::common::utils::test_txs;
use common::builders::contract::ERC20Token;
use revm::primitives::{
    uint, Account, AccountInfo, Address, BlockEnv, SpecId, TransactTo, TxEnv, U256,
};

fn generate_addresses(length: usize) -> Vec<Address> {
    (0..length).map(|_| Address::new(rand::random())).collect()
}

fn generate_cluster(
    num_families: usize,
    num_people_per_family: usize,
    num_transfers_per_person: usize,
) -> (Vec<(Address, Account)>, Vec<TxEnv>) {
    let families: Vec<Vec<Address>> = (0..num_families)
        .map(|_| generate_addresses(num_people_per_family))
        .collect();

    let people_addresses: Vec<Address> = families.clone().into_iter().flatten().collect();

    let gld_address = Address::new(rand::random());

    let gld_account = ERC20Token::new("Gold Token", "GLD", 18, 222_222_000_000_000_000_000_000u128)
        .add_balances(&people_addresses, uint!(1_000_000_000_000_000_000_U256))
        .build();

    let mut state = Vec::from(&[(gld_address, gld_account)]);
    let mut txs = Vec::new();

    for person in people_addresses.iter() {
        let info = AccountInfo::from_balance(uint!(4_567_000_000_000_000_000_000_U256));
        state.push((*person, Account::from(info)));
    }

    for nonce in 0..num_transfers_per_person {
        for family in families.iter() {
            for person in family {
                let recipient = family[(rand::random::<usize>()) % (family.len())];
                let calldata = ERC20Token::transfer(recipient, U256::from(rand::random::<u8>()));

                txs.push(TxEnv {
                    caller: *person,
                    gas_limit: 16_777_216u64,
                    gas_price: U256::from(0xb2d05e07u64),
                    transact_to: TransactTo::Call(gld_address),
                    value: U256::ZERO,
                    data: calldata,
                    nonce: Some(nonce as u64),
                    ..TxEnv::default()
                })
            }
        }
    }

    (state, txs)
}

#[test]
fn erc20_independent() {
    const N: usize = 1024;
    let (mut state, txs) = generate_cluster(N, 1, 1);
    state.push((Address::ZERO, Account::default()));
    test_txs(&state, SpecId::LATEST, BlockEnv::default(), txs);
}

#[test]
fn erc20_clusters() {
    const NUM_CLUSTERS: usize = 8;
    const NUM_FAMILIES_PER_CLUSTER: usize = 16;
    const NUM_PEOPLE_PER_FAMILY: usize = 6;
    const NUM_TRANSFERS_PER_PERSON: usize = 12;

    let mut final_state = Vec::from(&[(Address::ZERO, Account::default())]);
    let mut final_txs = Vec::<TxEnv>::new();
    for _ in 0..NUM_CLUSTERS {
        let (state, txs) = generate_cluster(
            NUM_FAMILIES_PER_CLUSTER,
            NUM_PEOPLE_PER_FAMILY,
            NUM_TRANSFERS_PER_PERSON,
        );
        final_state.extend(state);
        final_txs.extend(txs);
    }
    common::test_txs(&final_state, SpecId::LATEST, BlockEnv::default(), final_txs)
}
