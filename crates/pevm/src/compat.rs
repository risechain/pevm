// TODO: Support custom chains like OP & RISE
// Ideally REVM & Alloy would provide all these.

use alloy_primitives::U256;
use alloy_rpc_types_eth::Header;
use revm::{
    context::BlockEnv,
    context_interface::block::BlobExcessGasAndPrice,
    primitives::{
        eip4844::{BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE},
        hardfork::SpecId,
    },
};

/// Get the REVM block env of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L23-L48
// TODO: Better error handling & add tests, especially for [blob_excess_gas_and_price].
pub(crate) fn get_block_env(header: &Header, spec_id: impl Into<SpecId>) -> BlockEnv {
    let spec_id = spec_id.into();
    BlockEnv {
        number: U256::from(header.number),
        beneficiary: header.beneficiary,
        timestamp: U256::from(header.timestamp),
        gas_limit: header.gas_limit,
        basefee: header.base_fee_per_gas.unwrap_or_default(),
        difficulty: header.difficulty,
        prevrandao: Some(header.mix_hash),
        blob_excess_gas_and_price: header.excess_blob_gas.map(|excess_blob_gas| {
            BlobExcessGasAndPrice::new(
                excess_blob_gas,
                if spec_id.is_enabled_in(SpecId::PRAGUE) {
                    BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE
                } else {
                    BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN
                },
            )
        }),
    }
}
