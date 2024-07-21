//! Ethereum

use std::{collections::HashMap, fmt::Debug};

use alloy_chains::NamedChain;
use alloy_consensus::TxType;
use alloy_primitives::U256;
use alloy_rpc_types::{Header, Transaction};
use revm::primitives::{BlockEnv, SpecId, TxEnv};

use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    BuildIdentityHasher, MemoryLocation, TxIdx,
};

use super::PevmChain;

/// Implementation of [PevmChain] for Ethereum
#[derive(Debug, Clone, PartialEq)]
pub struct PevmEthereum {
    id: u64,
}

impl PevmEthereum {
    /// Ethereum Mainnet
    pub fn mainnet() -> Self {
        Self {
            id: NamedChain::Mainnet.into(),
        }
    }

    // TODO: support Ethereum Sepolia and other testnets
}

/// Error type for [PevmEthereum::get_block_spec].
#[derive(Debug, Clone, PartialEq)]
pub enum GetBlockSpecError {
    /// When [header.number] is none.
    MissingBlockNumber,
    /// When [header.total_difficulty] is none.
    MissingTotalDifficulty,
}

/// Error type for [PevmEthereum::get_gas_price].
#[derive(Debug, Clone, PartialEq)]
pub enum GetGasPriceError {
    /// [tx.type] is invalid.
    InvalidType(u8),
    /// [tx.gas_price] is none.
    MissingGasPrice,
    /// [tx.max_fee_per_gas] is none.
    MissingMaxFeePerGas,
}

impl PevmChain for PevmEthereum {
    type GetBlockSpecError = GetBlockSpecError;
    type GetGasPriceError = GetGasPriceError;

    fn id(&self) -> u64 {
        self.id
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from these reference:
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
    // TODO: Better error handling & properly test this.
    // TODO: Only Ethereum Mainnet is supported at the moment.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::GetBlockSpecError> {
        let number = header.number.ok_or(GetBlockSpecError::MissingBlockNumber)?;
        let total_difficulty = header
            .total_difficulty
            .ok_or(GetBlockSpecError::MissingTotalDifficulty)?;

        Ok(if header.timestamp >= 1710338135 {
            SpecId::CANCUN
        } else if header.timestamp >= 1681338455 {
            SpecId::SHANGHAI
        } else if total_difficulty.saturating_sub(header.difficulty)
            >= U256::from(58_750_000_000_000_000_000_000_u128)
        {
            SpecId::MERGE
        } else if number >= 12965000 {
            SpecId::LONDON
        } else if number >= 12244000 {
            SpecId::BERLIN
        } else if number >= 9069000 {
            SpecId::ISTANBUL
        } else if number >= 7280000 {
            SpecId::PETERSBURG
        } else if number >= 4370000 {
            SpecId::BYZANTIUM
        } else if number >= 2675000 {
            SpecId::SPURIOUS_DRAGON
        } else if number >= 2463000 {
            SpecId::TANGERINE
        } else if number >= 1150000 {
            SpecId::HOMESTEAD
        } else {
            SpecId::FRONTIER
        })
    }

    fn get_gas_price(&self, tx: &Transaction) -> Result<U256, Self::GetGasPriceError> {
        let tx_type_raw: u8 = tx.transaction_type.unwrap_or_default();
        let Ok(tx_type) = TxType::try_from(tx_type_raw) else {
            return Err(GetGasPriceError::InvalidType(tx_type_raw));
        };

        match tx_type {
            TxType::Legacy | TxType::Eip2930 => tx
                .gas_price
                .map(U256::from)
                .ok_or(GetGasPriceError::MissingGasPrice),
            TxType::Eip1559 | TxType::Eip4844 => tx
                .max_fee_per_gas
                .map(U256::from)
                .ok_or(GetGasPriceError::MissingMaxFeePerGas),
        }
    }

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
