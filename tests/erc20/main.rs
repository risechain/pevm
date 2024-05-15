// Each cluster has one ERC20 contract and X families.
// Each family has Y people.
// Each person performs Z transfers to random people within the family.

#[path = "../common/mod.rs"]
pub mod common;

#[path = "./mod.rs"]
pub mod erc20;

use common::test_execute_revm;
use erc20::generate_cluster;
use revm::{
    db::PlainAccount,
    primitives::{Address, BlockEnv, SpecId, TxEnv},
};

#[test]
fn erc20_independent() {
    const N: usize = 1024;
    let (mut state, txs) = generate_cluster(N, 1, 1);
    state.push((Address::ZERO, PlainAccount::default()));
    test_execute_revm(
        common::build_inmem_db(state),
        SpecId::LATEST,
        BlockEnv::default(),
        txs,
    );
}

#[test]
fn erc20_clusters() {
    const NUM_CLUSTERS: usize = 8;
    const NUM_FAMILIES_PER_CLUSTER: usize = 16;
    const NUM_PEOPLE_PER_FAMILY: usize = 6;
    const NUM_TRANSFERS_PER_PERSON: usize = 12;

    let mut final_state = Vec::from(&[(Address::ZERO, PlainAccount::default())]);
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
    common::test_execute_revm(
        common::build_inmem_db(final_state),
        SpecId::LATEST,
        BlockEnv::default(),
        final_txs,
    )
}
