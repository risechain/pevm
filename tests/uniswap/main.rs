//! Launch K clusters.
//! Each cluster has M people.
//! Each person makes N swaps.

#[path = "../common/mod.rs"]
pub mod common;

#[path = "../erc20/mod.rs"]
pub mod erc20;

#[path = "./mod.rs"]
pub mod uniswap;

use crate::uniswap::generate_cluster;
use revm::primitives::{Account, Address, BlockEnv, SpecId, TxEnv};

#[test]
fn uniswap_clusters() {
    const NUM_CLUSTERS: usize = 8;
    const NUM_PEOPLE_PER_CLUSTER: usize = 4;
    const NUM_SWAPS_PER_PERSON: usize = 16;

    let mut final_state = Vec::from(&[(Address::ZERO, Account::default())]);
    let mut final_txs = Vec::<TxEnv>::new();
    for _ in 0..NUM_CLUSTERS {
        let (state, txs) = generate_cluster(NUM_PEOPLE_PER_CLUSTER, NUM_SWAPS_PER_PERSON);
        final_state.extend(state);
        final_txs.extend(txs);
    }
    common::test_execute_revm(&final_state, SpecId::LATEST, BlockEnv::default(), final_txs)
}
