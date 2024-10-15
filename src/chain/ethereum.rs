//! Ethereum

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use alloy_chains::NamedChain;
use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::{B256, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{BlockTransactions, Header};
use revm::{
    primitives::{AuthorizationList, BlockEnv, SpecId, TxEnv},
    Handler,
};

use super::{CalculateReceiptRootError, PevmChain, RewardPolicy};
use crate::{
    mv_memory::MvMemory, BuildIdentityHasher, MemoryLocation, PevmTxExecutionResult, TxIdx,
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
    /// When [header.total_difficulty] is none.
    MissingTotalDifficulty,
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq)]
pub enum EthereumTransactionParsingError {
    /// [tx.type] is invalid.
    InvalidType(u8),
    /// [tx.gas_price] is none.
    MissingGasPrice,
    /// [tx.max_fee_per_gas] is none.
    MissingMaxFeePerGas,
}

fn get_ethereum_gas_price(
    tx: &alloy_rpc_types::Transaction,
) -> Result<U256, EthereumTransactionParsingError> {
    let tx_type_raw: u8 = tx.transaction_type.unwrap_or_default();
    let Ok(tx_type) = TxType::try_from(tx_type_raw) else {
        return Err(EthereumTransactionParsingError::InvalidType(tx_type_raw));
    };

    match tx_type {
        TxType::Legacy | TxType::Eip2930 => tx
            .gas_price
            .map(U256::from)
            .ok_or(EthereumTransactionParsingError::MissingGasPrice),
        TxType::Eip1559 | TxType::Eip4844 | TxType::Eip7702 => tx
            .max_fee_per_gas
            .map(U256::from)
            .ok_or(EthereumTransactionParsingError::MissingMaxFeePerGas),
    }
}

impl PevmChain for PevmEthereum {
    type Transaction = alloy_rpc_types::Transaction;
    type BlockSpecError = EthereumBlockSpecError;
    type TransactionParsingError = EthereumTransactionParsingError;

    fn id(&self) -> u64 {
        self.id
    }

    fn build_tx_from_alloy_tx(&self, tx: alloy_rpc_types::Transaction) -> Self::Transaction {
        tx
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from these reference:
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
    // TODO: Better error handling & properly test this.
    // TODO: Only Ethereum Mainnet is supported at the moment.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        Ok(if header.timestamp >= 1710338135 {
            SpecId::CANCUN
        } else if header.timestamp >= 1681338455 {
            SpecId::SHANGHAI
        } else if (header
            .total_difficulty
            .ok_or(EthereumBlockSpecError::MissingTotalDifficulty)?)
        .saturating_sub(header.difficulty)
            >= U256::from(58_750_000_000_000_000_000_000_u128)
        {
            SpecId::MERGE
        } else if header.number >= 12965000 {
            SpecId::LONDON
        } else if header.number >= 12244000 {
            SpecId::BERLIN
        } else if header.number >= 9069000 {
            SpecId::ISTANBUL
        } else if header.number >= 7280000 {
            SpecId::PETERSBURG
        } else if header.number >= 4370000 {
            SpecId::BYZANTIUM
        } else if header.number >= 2675000 {
            SpecId::SPURIOUS_DRAGON
        } else if header.number >= 2463000 {
            SpecId::TANGERINE
        } else if header.number >= 1150000 {
            SpecId::HOMESTEAD
        } else {
            SpecId::FRONTIER
        })
    }

    /// Get the REVM tx envs of an Alloy block.
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L234-L339
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/alloy_compat.rs#L112-L233
    // TODO: Properly test this.
    fn get_tx_env(&self, tx: Self::Transaction) -> Result<TxEnv, EthereumTransactionParsingError> {
        Ok(TxEnv {
            caller: tx.from,
            gas_limit: tx.gas,
            gas_price: get_ethereum_gas_price(&tx)?,
            gas_priority_fee: tx.max_priority_fee_per_gas.map(U256::from),
            transact_to: tx.to.into(),
            value: tx.value,
            data: tx.input,
            nonce: Some(tx.nonce),
            chain_id: tx.chain_id,
            access_list: tx.access_list.unwrap_or_default().into(),
            blob_hashes: tx.blob_versioned_hashes.unwrap_or_default(),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas.map(U256::from),
            authorization_list: Some(AuthorizationList::Signed(
                tx.authorization_list.unwrap_or_default(),
            )), // TODO: Support in the upcoming hardfork
            #[cfg(feature = "optimism")]
            optimism: revm::primitives::OptimismFields::default(),
        })
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

        MvMemory::new(block_size, estimated_locations, [block_env.coinbase])
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
        spec_id: SpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError> {
        if spec_id < SpecId::BYZANTIUM {
            // We can only calculate the receipts root from Byzantium.
            // Before EIP-658 (https://eips.ethereum.org/EIPS/eip-658), the
            // receipt root is calculated with the post transaction state root,
            // which we don't have here.

            // TODO: Allow to calculate the receipt root by providing the post
            // transaction state root.
            return Err(CalculateReceiptRootError::Unsupported);
        }

        // 1. Create a [Vec<TxType>]
        let tx_types: Vec<TxType> = txs
            .txns()
            .map(|tx| {
                let byte = tx.transaction_type.unwrap_or_default();
                TxType::try_from(byte).map_err(|_| CalculateReceiptRootError::InvalidTxType(byte))
            })
            .collect::<Result<_, _>>()?;

        // 2. Create an iterator of [ReceiptEnvelope]
        let receipt_envelope_iter =
            Iterator::zip(tx_types.iter(), tx_results.iter()).map(|(tx_type, tx_result)| {
                let receipt = tx_result.receipt.clone().with_bloom();
                match tx_type {
                    TxType::Legacy => ReceiptEnvelope::Legacy(receipt),
                    TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt),
                    TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt),
                    TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt),
                    TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt),
                }
            });

        // 3. Create a trie then calculate the root hash
        // We use [BTreeMap] because the keys must be sorted in ascending order.
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
        Ok(hash_builder.root())
    }

    fn is_eip_1559_enabled(&self, spec_id: SpecId) -> bool {
        spec_id >= SpecId::LONDON
    }

    fn is_eip_161_enabled(&self, spec_id: SpecId) -> bool {
        spec_id >= SpecId::SPURIOUS_DRAGON
    }
}
