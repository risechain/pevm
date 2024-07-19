//! Network specific utils

use std::fmt::Debug;

use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::SpecId;

/// A chain ID (u64) associated with relevant utils.
pub trait PevmChain {
    /// The error type for [Self::get_block_spec].
    type GetBlockSpecError: Debug + Clone;

    /// The error type for [Self::get_gas_price].
    type GetGasPriceError: Debug + Clone;

    /// Get chain id.
    fn id(&self) -> u64;

    /// Get block's [SpecId]
    fn get_block_spec(header: &Header) -> Result<SpecId, Self::GetBlockSpecError>;

    /// Get tx gas price.
    fn get_gas_price(tx: &Transaction) -> Result<U256, Self::GetGasPriceError>;
}

pub mod ethereum;
