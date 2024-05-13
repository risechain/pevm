pub mod common;
pub mod erc20;
pub mod transfer;
pub mod uniswap;

use revm::{
    db::PlainAccount,
    primitives::{Address, BlockEnv, SpecId, TxEnv},
};

#[test]
fn mixed() {
    const NUM_TRANSFER_CLUSTERS: usize = 16;
    const NUM_ERC20_CLUSTERS: usize = 16;
    const NUM_UNISWAP_CLUSTERS: usize = 16;

    let mut final_state = Vec::from(&[(Address::ZERO, PlainAccount::default())]);
    let mut final_txs = Vec::<TxEnv>::new();

    let (state, txs) = crate::transfer::generate_clusters(NUM_TRANSFER_CLUSTERS, 8, 8);
    final_state.extend(state);
    final_txs.extend(txs);

    let (state, txs) = crate::uniswap::generate_clusters(NUM_UNISWAP_CLUSTERS, 8, 8);
    final_state.extend(state);
    final_txs.extend(txs);

    let (state, txs) = crate::erc20::generate_clusters(NUM_ERC20_CLUSTERS, 8, 8);
    final_state.extend(state);
    final_txs.extend(txs);

    common::test_execute_revm(
        common::build_inmem_db(final_state),
        SpecId::LATEST,
        BlockEnv::default(),
        final_txs,
    )
}
