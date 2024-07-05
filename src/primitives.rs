// TODO: Support custom chains like OP & RISE
// Ideally REVM & Alloy would provide all these.

use alloy_chains::Chain;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, SpecId, TransactTo, TxEnv, U256};

#[derive(Debug, PartialEq, Clone)]
pub enum BlockSpecDerivationError {
    MissingTotalDifficulty,
    MissingBlockNumber,
}

/// Get the REVM spec id of an Alloy block.
// Currently hardcoding Ethereum hardforks from these reference:
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
// TODO: Better error handling & properly test this.

pub fn get_block_spec(
    _chain: Chain, // TODO: handle differently for different chain
    header: &Header,
) -> Result<SpecId, BlockSpecDerivationError> {
    let timestamp = header.timestamp;
    let block_number = header
        .number
        .ok_or(BlockSpecDerivationError::MissingBlockNumber)?;
    let total_difficulty = header
        .total_difficulty
        .ok_or(BlockSpecDerivationError::MissingTotalDifficulty)?;

    Ok(if timestamp >= 1710338135 {
        SpecId::CANCUN
    } else if timestamp >= 1681338455 {
        SpecId::SHANGHAI
    } else if total_difficulty.saturating_sub(header.difficulty)
        >= U256::from(58_750_000_000_000_000_000_000_u128)
    {
        SpecId::MERGE
    } else if block_number >= 12965000 {
        SpecId::LONDON
    } else if block_number >= 12244000 {
        SpecId::BERLIN
    } else if block_number >= 9069000 {
        SpecId::ISTANBUL
    } else if block_number >= 7280000 {
        SpecId::PETERSBURG
    } else if block_number >= 4370000 {
        SpecId::BYZANTIUM
    } else if block_number >= 2675000 {
        SpecId::SPURIOUS_DRAGON
    } else if block_number >= 2463000 {
        SpecId::TANGERINE
    } else if block_number >= 1150000 {
        SpecId::HOMESTEAD
    } else {
        SpecId::FRONTIER
    })
}

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
    MissingGasPrice,
    MissingMaxFeePerGas,
    InvalidType(u8),
}

/// Get the REVM tx envs of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L234-L339
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/alloy_compat.rs#L112-L233
// TODO: Properly test this.
pub(crate) fn get_tx_env(tx: Transaction) -> Result<TxEnv, TransactionParsingError> {
    Ok(TxEnv {
        caller: tx.from,
        gas_limit: tx
            .gas
            .try_into()
            .map_err(|_| TransactionParsingError::OverflowedGasLimit)?,
        gas_price: match tx.transaction_type.unwrap() {
            0 | 1 => U256::from(
                tx.gas_price
                    .ok_or(TransactionParsingError::MissingGasPrice)?,
            ),
            2 | 3 => U256::from(
                tx.max_fee_per_gas
                    .ok_or(TransactionParsingError::MissingMaxFeePerGas)?,
            ),
            unknown => return Err(TransactionParsingError::InvalidType(unknown)),
        },
        gas_priority_fee: tx.max_priority_fee_per_gas.map(U256::from),
        transact_to: match tx.to {
            Some(address) => TransactTo::Call(address),
            None => TransactTo::Create,
        },
        value: tx.value,
        data: tx.input,
        nonce: Some(tx.nonce),
        chain_id: tx.chain_id,
        access_list: tx
            .access_list
            .unwrap_or_default()
            .iter()
            .map(|access| {
                (
                    access.address,
                    access
                        .storage_keys
                        .iter()
                        .map(|&k| U256::from_be_bytes(*k))
                        .collect(),
                )
            })
            .collect(),
        blob_hashes: tx.blob_versioned_hashes.unwrap_or_default(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas.map(U256::from),
    })
}
