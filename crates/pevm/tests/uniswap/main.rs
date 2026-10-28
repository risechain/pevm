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
use common::MockEvmTx;
use pevm::chain::{PevmEthereum, PevmRise};
use pevm::{Bytecodes, ChainState, EvmAccount, InMemoryStorage};
use revm::context::TxEnv;
use revm::primitives::Address;
use std::sync::Arc;

fn test_uniswap_clusters<C>(chain: &C)
where
    C: MockEvmTx + PartialEq + Send + Sync,
{
    const NUM_CLUSTERS: usize = 20;
    const NUM_PEOPLE_PER_CLUSTER: usize = 20;
    const NUM_SWAPS_PER_PERSON: usize = 20;

    let mut final_state = ChainState::default();
    final_state.insert(Address::ZERO, EvmAccount::default()); // Beneficiary
    let mut final_bytecodes = Bytecodes::default();
    let mut final_txs = Vec::<TxEnv>::new();
    for _ in 0..NUM_CLUSTERS {
        let (state, bytecodes, txs) =
            generate_cluster(NUM_PEOPLE_PER_CLUSTER, NUM_SWAPS_PER_PERSON);
        final_state.extend(state);
        final_bytecodes.extend(bytecodes);
        final_txs.extend(txs);
    }
    common::test_execute_revm(
        chain,
        InMemoryStorage::new(final_state, Arc::new(final_bytecodes), Default::default()),
        final_txs
            .into_iter()
            .map(|tx| chain.mock_evm_tx(tx))
            .collect(),
    )
}

#[test]
fn ethereum_uniswap_clusters() {
    test_uniswap_clusters(&PevmEthereum::mainnet());
}

#[test]
fn rise_uniswap_clusters() {
    test_uniswap_clusters(&PevmRise);
}
