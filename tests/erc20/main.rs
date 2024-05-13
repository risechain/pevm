// X clusters share one ERC20 contract.
// Each cluster has Y people.
// Each person performs Z transfers to random people within the cluster.

#[path = "../common/mod.rs"]
pub mod common;

#[path = "./mod.rs"]
pub mod erc20;

use common::test_execute_revm;
use erc20::generate_clusters;
use revm::{
    db::PlainAccount,
    primitives::{Address, BlockEnv, SpecId},
};

#[test]
fn erc20_independent() {
    const NUM_TRANSFERS: usize = 1024;
    let (mut state, txs) = generate_clusters(NUM_TRANSFERS, 1, 1);
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
    const NUM_PEOPLE_PER_CLUSTER: usize = 6;
    const NUM_TRANSFERS_PER_PERSON: usize = 12;

    let (mut state, txs) = generate_clusters(
        NUM_CLUSTERS,
        NUM_PEOPLE_PER_CLUSTER,
        NUM_TRANSFERS_PER_PERSON,
    );

    state.push((Address::ZERO, PlainAccount::default()));

    common::test_execute_revm(
        common::build_inmem_db(state),
        SpecId::LATEST,
        BlockEnv::default(),
        txs,
    )
}
