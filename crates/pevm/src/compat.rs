// TODO: Support custom chains like OP & RISE
// Ideally REVM & Alloy would provide all these.

use alloy_rpc_types_eth::Header;
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, U256};

/// Get the REVM block env of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L23-L48
// TODO: Better error handling & properly test this, especially
// [blob_excess_gas_and_price].
pub(crate) fn get_block_env(header: &Header) -> BlockEnv {
    BlockEnv {
        number: U256::from(header.number),
        coinbase: header.miner,
        timestamp: U256::from(header.timestamp),
        gas_limit: U256::from(header.gas_limit),
        basefee: U256::from(header.base_fee_per_gas.unwrap_or_default()),
        difficulty: header.difficulty,
        prevrandao: header.mix_hash,
        blob_excess_gas_and_price: header.excess_blob_gas.map(BlobExcessGasAndPrice::new),
    }
}
