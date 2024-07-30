//! Ethereum

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use alloy_chains::NamedChain;
use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::{B256, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{BlockTransactions, Header, Transaction};
use revm::{
    primitives::{BlockEnv, SpecId, TxEnv},
    Handler,
};

use super::{PevmChain, RewardPolicy};
use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    BuildIdentityHasher, MemoryLocation, PevmTxExecutionResult, TxIdx,
};

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
pub enum EthereumBlockSpecError {
    /// When [header.number] is none.
    MissingBlockNumber,
    /// When [header.total_difficulty] is none.
    MissingTotalDifficulty,
}

/// Error type for [PevmEthereum::get_gas_price].
#[derive(Debug, Clone, PartialEq)]
pub enum EthereumGasPriceError {
    /// [tx.type] is invalid.
    InvalidType(u8),
    /// [tx.gas_price] is none.
    MissingGasPrice,
    /// [tx.max_fee_per_gas] is none.
    MissingMaxFeePerGas,
}

impl PevmChain for PevmEthereum {
    type BlockSpecError = EthereumBlockSpecError;
    type GasPriceError = EthereumGasPriceError;

    fn id(&self) -> u64 {
        self.id
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from these reference:
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
    // TODO: Better error handling & properly test this.
    // TODO: Only Ethereum Mainnet is supported at the moment.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        let number = header
            .number
            .ok_or(EthereumBlockSpecError::MissingBlockNumber)?;
        let total_difficulty = header
            .total_difficulty
            .ok_or(EthereumBlockSpecError::MissingTotalDifficulty)?;

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

    fn get_gas_price(&self, tx: &Transaction) -> Result<U256, Self::GasPriceError> {
        let tx_type_raw: u8 = tx.transaction_type.unwrap_or_default();
        let Ok(tx_type) = TxType::try_from(tx_type_raw) else {
            return Err(EthereumGasPriceError::InvalidType(tx_type_raw));
        };

        match tx_type {
            TxType::Legacy | TxType::Eip2930 => tx
                .gas_price
                .map(U256::from)
                .ok_or(EthereumGasPriceError::MissingGasPrice),
            TxType::Eip1559 | TxType::Eip4844 => tx
                .max_fee_per_gas
                .map(U256::from)
                .ok_or(EthereumGasPriceError::MissingMaxFeePerGas),
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

    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB> {
        Handler::mainnet_with_spec(spec_id, with_reward_beneficiary)
    }

    fn get_reward_policy(&self, _hasher: &ahash::RandomState) -> RewardPolicy {
        RewardPolicy::Ethereum
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    fn calculate_receipt_root(
        &self,
        _spec_id: SpecId,
        txs: &BlockTransactions<Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> B256 {
        // 1. Create an iterator of ReceiptEnvelope
        let tx_type_iter = txs
            .txns()
            .map(|tx| TxType::try_from(tx.transaction_type.unwrap_or_default()).unwrap());

        let receipt_iter = tx_results.iter().map(|tx| tx.receipt.clone().with_bloom());

        let receipt_envelope_iter =
            Iterator::zip(tx_type_iter, receipt_iter).map(|(tx_type, receipt)| match tx_type {
                TxType::Legacy => ReceiptEnvelope::Legacy(receipt),
                TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt),
                TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt),
                TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt),
            });

        // 2. Create a trie then calculate the root hash
        // We use BTreeMap because the keys must be sorted in ascending order.
        let trie_entries: BTreeMap<_, _> = receipt_envelope_iter
            .enumerate()
            .map(|(index, receipt)| {
                let key_buffer = alloy_rlp::encode_fixed_size(&index);
                let mut value_buffer = Vec::new();
                receipt.encode_2718(&mut value_buffer);
                (key_buffer, value_buffer)
            })
            .collect();

        let mut hash_builder = alloy_trie::HashBuilder::default();
        for (k, v) in trie_entries {
            hash_builder.add_leaf(alloy_trie::Nibbles::unpack(&k), &v);
        }
        hash_builder.root()
    }
}
