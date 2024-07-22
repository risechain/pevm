//! Optimism
#![allow(missing_docs)]
use std::collections::{BTreeMap, HashMap};

use alloy_chains::NamedChain;
use alloy_consensus::{Signed, TxEip1559, TxEip2930, TxEip4844, TxLegacy};
use alloy_primitives::{Bytes, B256, U128, U256};
use alloy_rpc_types::{BlockTransactions, Header, Transaction};
use op_alloy_consensus::{OpDepositReceipt, OpReceiptEnvelope, OpTxEnvelope, OpTxType, TxDeposit};
use op_alloy_network::eip2718::Encodable2718;
use revm::{
    primitives::{BlockEnv, OptimismFields, SpecId, TxEnv},
    Handler,
};

use crate::{
    mv_memory::{LazyAddresses, MvMemory},
    BuildIdentityHasher, MemoryLocation, PevmTxExecutionResult, TxIdx,
};

use super::{PevmChain, RewardPolicy};

/// Error when converting [Transaction] to [OptimismFields]
#[derive(Debug, Clone, PartialEq)]
pub enum OptimismFieldsConversionError {
    MissingSourceHash,
    UnexpectedType(u8),
    SerdeError(String),
    ConversionError(String),
}

/// Convert [Transaction] to [OptimismFields]
pub(crate) fn get_optimism_fields(
    tx: Transaction,
) -> Result<OptimismFields, OptimismFieldsConversionError> {
    let source_hash = tx
        .other
        .get_deserialized::<B256>("sourceHash")
        .transpose()
        .map_err(|err| OptimismFieldsConversionError::SerdeError(err.to_string()))?;
    let mint = tx
        .other
        .get_deserialized::<U128>("mint")
        .transpose()
        .map_err(|err| OptimismFieldsConversionError::SerdeError(err.to_string()))?;
    let is_system_transaction = tx
        .other
        .get_deserialized("isSystemTx")
        .transpose()
        .map_err(|err| OptimismFieldsConversionError::SerdeError(err.to_string()))?;

    let envelope_buf = {
        let tx_type = tx.transaction_type.unwrap_or_default();
        let op_tx_type = OpTxType::try_from(tx_type)
            .map_err(|_err| OptimismFieldsConversionError::UnexpectedType(tx_type))?;
        let tx_envelope = match op_tx_type {
            OpTxType::Legacy => Signed::<TxLegacy>::try_from(tx.clone()).map(OpTxEnvelope::from),
            OpTxType::Eip2930 => Signed::<TxEip2930>::try_from(tx.clone()).map(OpTxEnvelope::from),
            OpTxType::Eip1559 => Signed::<TxEip1559>::try_from(tx.clone()).map(OpTxEnvelope::from),
            OpTxType::Eip4844 => Signed::<TxEip4844>::try_from(tx.clone()).map(OpTxEnvelope::from),
            OpTxType::Deposit => {
                let tx_deposit = TxDeposit {
                    source_hash: source_hash
                        .ok_or(OptimismFieldsConversionError::MissingSourceHash)?,
                    from: tx.from,
                    to: tx.to.into(),
                    mint: mint.map(|x| x.to()),
                    value: tx.value,
                    gas_limit: tx.gas,
                    is_system_transaction: is_system_transaction.unwrap_or_default(),
                    input: tx.input.clone(),
                };
                Ok(OpTxEnvelope::from(tx_deposit))
            }
        }
        .map_err(|err| OptimismFieldsConversionError::ConversionError(err.to_string()))?;

        let mut envelope_buf = Vec::<u8>::new();
        tx_envelope.encode_2718(&mut envelope_buf);
        Bytes::from(envelope_buf)
    };

    Ok(OptimismFields {
        source_hash,
        mint: mint.map(|x| x.to()),
        is_system_transaction,
        enveloped_tx: Some(envelope_buf),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimismBlockSpecError {
    MissingBlockNumber,
    UnsupportedSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimismGasPriceError {
    InvalidType(u8),
    MissingGasPrice,
    MissingMaxFeePerGas,
}

// https://github.com/paradigmxyz/reth/blob/b4a1b733c93f7e262f1b774722670e08cdcb6276/crates/primitives/src/proofs.rs
fn encode_receipt_2718(
    spec_id: SpecId,
    tx: &Transaction,
    tx_result: &PevmTxExecutionResult,
) -> Bytes {
    let tx_type = tx.transaction_type.unwrap_or_default();
    let op_tx_type = OpTxType::try_from(tx_type).unwrap();
    let receipt_envelope = match op_tx_type {
        OpTxType::Legacy => OpReceiptEnvelope::Legacy(tx_result.receipt.clone().with_bloom()),
        OpTxType::Eip2930 => OpReceiptEnvelope::Eip2930(tx_result.receipt.clone().with_bloom()),
        OpTxType::Eip1559 => OpReceiptEnvelope::Eip1559(tx_result.receipt.clone().with_bloom()),
        OpTxType::Eip4844 => OpReceiptEnvelope::Eip4844(tx_result.receipt.clone().with_bloom()),
        OpTxType::Deposit => {
            let account_maybe = tx_result.state.get(&tx.from).expect("Sender not found");
            let account = account_maybe.as_ref().expect("Sender not changed");
            let receipt = OpDepositReceipt {
                inner: tx_result.receipt.clone(),
                deposit_nonce: (spec_id >= SpecId::CANYON).then_some(account.basic.nonce - 1),
                deposit_receipt_version: (spec_id >= SpecId::CANYON).then_some(1),
            };
            OpReceiptEnvelope::Deposit(receipt.with_bloom())
        }
    };

    let mut buffer = Vec::new();
    receipt_envelope.encode_2718(&mut buffer);
    Bytes::from(buffer)
}

/// Implementation of [PevmChain] for Ethereum
#[derive(Debug, Clone, PartialEq)]
pub struct PevmOptimism {
    id: u64,
}

impl PevmOptimism {
    pub fn mainnet() -> Self {
        PevmOptimism {
            id: NamedChain::Optimism.into(),
        }
    }
}

impl PevmChain for PevmOptimism {
    type BlockSpecError = OptimismBlockSpecError;

    type GasPriceError = OptimismGasPriceError;

    fn id(&self) -> u64 {
        self.id
    }

    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        let timestamp = header.timestamp;
        let block_number = header
            .number
            .ok_or(OptimismBlockSpecError::MissingBlockNumber)?;

        if timestamp >= 1720627201 {
            Ok(SpecId::FJORD)
        } else if timestamp >= 1710374401 {
            Ok(SpecId::ECOTONE)
        } else if timestamp >= 1704992401 {
            Ok(SpecId::CANYON)
        } else if block_number >= 105235063 {
            Ok(SpecId::REGOLITH)
        } else {
            // TODO: revm does not support when L1Block is not available
            Err(OptimismBlockSpecError::UnsupportedSpec)
        }
    }

    fn get_gas_price(&self, tx: &Transaction) -> Result<U256, Self::GasPriceError> {
        let tx_type_raw = tx.transaction_type.unwrap_or_default();
        let Ok(tx_type) = OpTxType::try_from(tx_type_raw) else {
            return Err(OptimismGasPriceError::InvalidType(tx_type_raw));
        };

        match tx_type {
            OpTxType::Legacy | OpTxType::Eip2930 => tx
                .gas_price
                .map(U256::from)
                .ok_or(OptimismGasPriceError::MissingGasPrice),
            OpTxType::Eip1559 | OpTxType::Eip4844 => tx
                .max_fee_per_gas
                .map(U256::from)
                .ok_or(OptimismGasPriceError::MissingMaxFeePerGas),
            OpTxType::Deposit => Ok(U256::ZERO),
        }
    }

    fn build_mv_memory(
        &self,
        hasher: &ahash::RandomState,
        block_env: &BlockEnv,
        txs: &[TxEnv],
    ) -> MvMemory {
        let beneficiary_location_hash = hasher.hash_one(MemoryLocation::Basic(block_env.coinbase));

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        estimated_locations.insert(
            beneficiary_location_hash,
            txs.iter()
                .enumerate()
                .filter_map(|(index, tx)| tx.optimism.source_hash.is_none().then_some(index))
                .collect::<Vec<TxIdx>>(),
        );

        let mut lazy_addresses = LazyAddresses::default();
        lazy_addresses.0.extend(vec![
            block_env.coinbase,
            revm::L1_FEE_RECIPIENT,
            revm::BASE_FEE_RECIPIENT,
        ]);

        MvMemory::new(txs.len(), estimated_locations, lazy_addresses)
    }

    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB> {
        Handler::optimism_with_spec(spec_id, with_reward_beneficiary)
    }

    fn get_reward_policy(&self, hasher: &ahash::RandomState) -> RewardPolicy {
        RewardPolicy::Optimism {
            l1_fee_recipient_location_hash: hasher
                .hash_one(MemoryLocation::Basic(revm::optimism::L1_FEE_RECIPIENT)),
            base_fee_vault_location_hash: hasher
                .hash_one(MemoryLocation::Basic(revm::optimism::BASE_FEE_RECIPIENT)),
        }
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    fn calculate_receipt_root(
        &self,
        spec_id: SpecId,
        txs: &BlockTransactions<Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> B256 {
        let trie_entries: BTreeMap<_, _> = txs
            .txns()
            .zip(tx_results)
            .enumerate()
            .map(|(index, (tx, tx_result))| {
                let key_buffer = alloy_rlp::encode_fixed_size(&index).to_vec();
                let value_buffer = encode_receipt_2718(spec_id, tx, tx_result);
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
