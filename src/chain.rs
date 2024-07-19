//! Chain specific utils

use std::{collections::HashMap, fmt::Debug};

use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlockEnv, SpecId, TxEnv};

use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    BuildIdentityHasher, MemoryLocation, TxIdx,
};

/// Custom behaviours for different chains & networks
pub trait PevmChain: Debug + Clone + PartialEq {
    /// The error type for [Self::get_block_spec].
    type GetBlockSpecError: Debug + Clone + PartialEq;

    /// The error type for [Self::get_gas_price].
    type GetGasPriceError: Debug + Clone + PartialEq;

    /// Get chain id.
    fn id(&self) -> u64;

    /// Get block's [SpecId]
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::GetBlockSpecError>;

    /// Get tx gas price.
    fn get_gas_price(&self, tx: &Transaction) -> Result<U256, Self::GetGasPriceError>;

    /// Build [MvMemory]
    fn build_mv_memory(
        &self,
        hasher: &ahash::RandomState,
        block_env: &BlockEnv,
        txs: &[TxEnv],
    ) -> MvMemory {
        let block_size = txs.len();
        let beneficiary_location_hash = hasher.hash_one(MemoryLocation::Basic(block_env.coinbase));

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        estimated_locations.insert(
            beneficiary_location_hash,
            (0..block_size).collect::<Vec<TxIdx>>(),
        );

        let mut lazy_addresses = LazyAddresses::default();
        lazy_addresses.0.insert(block_env.coinbase);

        MvMemory::new(block_size, estimated_locations, lazy_addresses)
    }
}

mod ethereum;
pub use ethereum::PevmEthereum;
