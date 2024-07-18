//! Ethereum
use std::collections::HashMap;

use alloy_consensus::TxType;
use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlockEnv, SpecId};

use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    primitives::TransactionParsingError,
    BuildIdentityHasher, MemoryLocation, TxIdx,
};

/// Build MvMemory
pub(crate) fn build_mv_memory(
    hasher: &ahash::RandomState,
    block_env: &BlockEnv,
    block_size: usize,
) -> MvMemory {
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

/// Get the REVM spec id of an Alloy block.
// Currently hardcoding Ethereum hardforks from these reference:
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
// TODO: Better error handling & properly test this.
pub fn get_block_spec(header: &Header) -> Option<SpecId> {
    Some(if header.timestamp >= 1710338135 {
        SpecId::CANCUN
    } else if header.timestamp >= 1681338455 {
        SpecId::SHANGHAI
    } else if header.total_difficulty?.saturating_sub(header.difficulty)
        >= U256::from(58_750_000_000_000_000_000_000_u128)
    {
        SpecId::MERGE
    } else if header.number? >= 12965000 {
        SpecId::LONDON
    } else if header.number? >= 12244000 {
        SpecId::BERLIN
    } else if header.number? >= 9069000 {
        SpecId::ISTANBUL
    } else if header.number? >= 7280000 {
        SpecId::PETERSBURG
    } else if header.number? >= 4370000 {
        SpecId::BYZANTIUM
    } else if header.number? >= 2675000 {
        SpecId::SPURIOUS_DRAGON
    } else if header.number? >= 2463000 {
        SpecId::TANGERINE
    } else if header.number? >= 1150000 {
        SpecId::HOMESTEAD
    } else {
        SpecId::FRONTIER
    })
}

pub(crate) fn get_gas_price(tx: &Transaction) -> Result<U256, TransactionParsingError> {
    let tx_type_raw: u8 = tx.transaction_type.unwrap_or_default();
    let Ok(tx_type) = TxType::try_from(tx_type_raw) else {
        return Err(TransactionParsingError::InvalidType(tx_type_raw));
    };

    match tx_type {
        TxType::Legacy | TxType::Eip2930 => tx
            .gas_price
            .map(U256::from)
            .ok_or(TransactionParsingError::MissingGasPrice),
        TxType::Eip1559 | TxType::Eip4844 => tx
            .max_fee_per_gas
            .map(U256::from)
            .ok_or(TransactionParsingError::MissingMaxFeePerGas),
    }
}
