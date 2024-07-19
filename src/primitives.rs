// TODO: Support custom chains like OP & RISE
// Ideally REVM & Alloy would provide all these.

use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, TransactTo, TxEnv, U256};

use crate::chain::PevmChain;

/// Get the REVM block env of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L23-L48
// TODO: Better error handling & properly test this, especially
// [blob_excess_gas_and_price].
pub(crate) fn get_block_env(header: &Header) -> Option<BlockEnv> {
    Some(BlockEnv {
        number: U256::from(header.number?),
        coinbase: header.miner,
        timestamp: U256::from(header.timestamp),
        gas_limit: U256::from(header.gas_limit),
        basefee: U256::from(header.base_fee_per_gas.unwrap_or_default()),
        difficulty: header.difficulty,
        prevrandao: header.mix_hash,
        blob_excess_gas_and_price: header
            .excess_blob_gas
            .map(|excess_blob_gas| BlobExcessGasAndPrice::new(excess_blob_gas as u64)),
    })
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionParsingError {
    OverflowedGasLimit,
    GetGasPriceError(String),
    MissingMaxFeePerGas,
    InvalidType(u8),
}

/// Get the REVM tx envs of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L234-L339
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/alloy_compat.rs#L112-L233
// TODO: Properly test this.
pub(crate) fn get_tx_env<C: PevmChain>(tx: Transaction) -> Result<TxEnv, TransactionParsingError> {
    Ok(TxEnv {
        caller: tx.from,
        gas_limit: tx
            .gas
            .try_into()
            .map_err(|_| TransactionParsingError::OverflowedGasLimit)?,
        gas_price: C::get_gas_price(&tx)
            .map_err(|err| TransactionParsingError::GetGasPriceError(format!("{:?}", err)))?,
        gas_priority_fee: tx.max_priority_fee_per_gas.map(U256::from),
        transact_to: match tx.to {
            Some(address) => TransactTo::Call(address),
            None => TransactTo::Create,
        },
        value: tx.value,
        data: tx.input,
        nonce: Some(tx.nonce),
        chain_id: tx.chain_id,
        access_list: tx.access_list.unwrap_or_default().0,
        blob_hashes: tx.blob_versioned_hashes.unwrap_or_default(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas.map(U256::from),
        authorization_list: None, // TODO: Support in the upcoming hardfork
    })
}
