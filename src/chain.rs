//! Chain specific utils

use std::fmt::Debug;

use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlockEnv, SpecId, TxEnv};

use crate::mv_memory::MvMemory;

/// Custom behaviours for different chains & networks
pub trait PevmChain {
    /// The error type for [Self::build_mv_memory].
    type BuildMvMemoryError: Debug + Clone;

    /// The error type for [Self::get_block_spec].
    type GetBlockSpecError: Debug + Clone;

    /// The error type for [Self::get_gas_price].
    type GetGasPriceError: Debug + Clone;

    /// Get chain id.
    fn id(&self) -> u64;

    /// Build [MvMemory]
    fn build_mv_memory(
        hasher: &ahash::RandomState,
        block_env: &BlockEnv,
        txs: &[TxEnv],
    ) -> Result<MvMemory, Self::BuildMvMemoryError>;

    /// Get block's [SpecId]
    fn get_block_spec(header: &Header) -> Result<SpecId, Self::GetBlockSpecError>;

    /// Get tx gas price.
    fn get_gas_price(tx: &Transaction) -> Result<U256, Self::GetGasPriceError>;
}

mod ethereum;
pub use ethereum::PevmEthereum;
