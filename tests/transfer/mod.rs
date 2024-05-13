use alloy_primitives::U256;
use revm::{
    db::PlainAccount,
    primitives::{uint, AccountInfo, Address, TransactTo, TxEnv},
};

fn generate_addresses(length: usize) -> Vec<Address> {
    (0..length).map(|_| Address::new(rand::random())).collect()
}

pub fn generate_clusters(
    num_clusters: usize,
    num_people_per_cluster: usize,
    num_transfers_per_person: usize,
) -> (Vec<(Address, PlainAccount)>, Vec<TxEnv>) {
    let clusters: Vec<Vec<Address>> = (0..num_clusters)
        .map(|_| generate_addresses(num_people_per_cluster))
        .collect();

    let people_addresses: Vec<Address> = clusters.clone().into_iter().flatten().collect();

    let mut state = Vec::new();
    let mut txs = Vec::new();

    for person in people_addresses.iter() {
        let info = AccountInfo::from_balance(uint!(4_567_000_000_000_000_000_000_U256));
        state.push((*person, PlainAccount::from(info)));
    }

    for nonce in 0..num_transfers_per_person {
        for cluster in clusters.iter() {
            for person in cluster {
                // send 1 wei to a random recipient within the cluster
                let recipient = cluster[(rand::random::<usize>()) % (cluster.len())];
                txs.push(TxEnv {
                    caller: *person,
                    gas_limit: 21_000u64,
                    gas_price: U256::from(0xd05e07b2u64),
                    transact_to: TransactTo::Call(recipient),
                    value: U256::from(1),
                    nonce: Some(nonce as u64),
                    ..TxEnv::default()
                })
            }
        }
    }

    (state, txs)
}
