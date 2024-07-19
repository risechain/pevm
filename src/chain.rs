//! Chain specific utils

use std::fmt::Debug;

use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlockEnv, SpecId, TxEnv};

use crate::mv_memory::MvMemory;

/// Custom behaviours for different chains & networks
pub trait PevmChain {
    /// The error type for [Self::get_block_spec].
    type GetBlockSpecError: Debug + Clone;

    /// The error type for [Self::get_gas_price].
    type GetGasPriceError: Debug + Clone;

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
    ) -> MvMemory;
}

mod ethereum;
pub use ethereum::PevmEthereum;
