//! Optimism
use std::collections::BTreeMap;

use alloy_chains::NamedChain;
use alloy_consensus::{Signed, TxEip1559, TxEip2930, TxEip7702, TxLegacy};
use alloy_primitives::{Bytes, B256, U256};
use alloy_rpc_types::{BlockTransactions, Header};
use hashbrown::HashMap;
use op_alloy_consensus::{OpDepositReceipt, OpReceiptEnvelope, OpTxEnvelope, OpTxType, TxDeposit};
use op_alloy_network::eip2718::Encodable2718;
use revm::{
    primitives::{AuthorizationList, BlockEnv, OptimismFields, SpecId, TxEnv},
    Handler,
};

use crate::{
    hash_determinisitic, mv_memory::MvMemory, BuildIdentityHasher, MemoryLocation,
    PevmTxExecutionResult,
};

use super::{CalculateReceiptRootError, PevmChain, RewardPolicy};

/// Implementation of [PevmChain] for Optimism
#[derive(Debug, Clone, PartialEq)]
pub struct PevmOptimism {
    id: u64,
}

impl PevmOptimism {
    /// Optimism Mainnet
    pub fn mainnet() -> Self {
        PevmOptimism {
            id: NamedChain::Optimism.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimismBlockSpecError {
    MissingBlockNumber,
    UnsupportedSpec,
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq)]
pub enum OptimismTransactionParsingError {
    ConversionError(String),
    InvalidType(u8),
    MissingGasPrice,
    MissingMaxFeePerGas,
    MissingSourceHash,
    SerdeError(String),
}

fn get_optimism_gas_price(
    tx: &op_alloy_rpc_types::Transaction,
) -> Result<U256, OptimismTransactionParsingError> {
    let tx_type_raw = tx.inner.transaction_type.unwrap_or_default();
    let Ok(tx_type) = OpTxType::try_from(tx_type_raw) else {
        return Err(OptimismTransactionParsingError::InvalidType(tx_type_raw));
    };

    match tx_type {
        OpTxType::Legacy | OpTxType::Eip2930 => tx
            .inner
            .gas_price
            .map(U256::from)
            .ok_or(OptimismTransactionParsingError::MissingGasPrice),
        OpTxType::Eip1559 | OpTxType::Eip7702 => tx
            .inner
            .max_fee_per_gas
            .map(U256::from)
            .ok_or(OptimismTransactionParsingError::MissingMaxFeePerGas),
        OpTxType::Deposit => Ok(U256::ZERO),
    }
}

/// Convert [Transaction] to [OptimismFields]
/// https://github.com/paradigmxyz/reth/blob/fc4c037e60b623b81b296fe9242fa905ff36b89a/crates/primitives/src/transaction/compat.rs#L99
pub(crate) fn get_optimism_fields(
    tx: &op_alloy_rpc_types::Transaction,
) -> Result<OptimismFields, OptimismTransactionParsingError> {
    let tx_type = tx.inner.transaction_type.unwrap_or_default();
    let op_tx_type = OpTxType::try_from(tx_type)
        .map_err(|_err| OptimismTransactionParsingError::InvalidType(tx_type))?;

    let inner = tx.inner.clone();
    let tx_envelope = match op_tx_type {
        OpTxType::Legacy => Signed::<TxLegacy>::try_from(inner).map(OpTxEnvelope::from),
        OpTxType::Eip2930 => Signed::<TxEip2930>::try_from(inner).map(OpTxEnvelope::from),
        OpTxType::Eip1559 => Signed::<TxEip1559>::try_from(inner).map(OpTxEnvelope::from),
        OpTxType::Eip7702 => Signed::<TxEip7702>::try_from(inner).map(OpTxEnvelope::from),
        OpTxType::Deposit => {
            let tx_deposit = TxDeposit {
                source_hash: tx
                    .source_hash
                    .ok_or(OptimismTransactionParsingError::MissingSourceHash)?,
                from: tx.inner.from,
                to: tx.inner.to.into(),
                mint: tx.mint,
                value: tx.inner.value,
                gas_limit: tx.inner.gas,
                is_system_transaction: tx.is_system_tx.unwrap_or_default(),
                input: tx.inner.input.clone(),
            };
            Ok(OpTxEnvelope::from(tx_deposit))
        }
    }
    .map_err(|err| OptimismTransactionParsingError::ConversionError(err.to_string()))?;

    let mut envelope_buf = Vec::<u8>::new();
    tx_envelope.encode_2718(&mut envelope_buf);

    Ok(OptimismFields {
        source_hash: tx.source_hash,
        mint: tx.mint,
        is_system_transaction: tx.is_system_tx,
        enveloped_tx: Some(Bytes::from(envelope_buf)),
    })
}

impl PevmChain for PevmOptimism {
    type Transaction = op_alloy_rpc_types::Transaction;
    type BlockSpecError = OptimismBlockSpecError;
    type TransactionParsingError = OptimismTransactionParsingError;

    fn id(&self) -> u64 {
        self.id
    }

    // TODO: allow to construct deposit transactions
    fn build_tx_from_alloy_tx(&self, tx: alloy_rpc_types::Transaction) -> Self::Transaction {
        Self::Transaction {
            inner: tx,
            mint: None,
            source_hash: None,
            is_system_tx: None,
            deposit_receipt_version: None,
        }
    }

    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        // TODO: The implementation below is only true for Optimism Mainnet.
        // When supporting other networks (e.g. Optimism Sepolia), remember to adjust the code here.
        if header.timestamp >= 1720627201 {
            Ok(SpecId::FJORD)
        } else if header.timestamp >= 1710374401 {
            Ok(SpecId::ECOTONE)
        } else if header.timestamp >= 1704992401 {
            Ok(SpecId::CANYON)
        } else if header.number >= 105235063 {
            // On Optimism Mainnet, Bedrock and Regolith are activated at the same time.
            // Therefore, this function never returns SpecId::BEDROCK.
            // The statement above might not be true for other network, e.g. Optimism Goerli.
            Ok(SpecId::REGOLITH)
        } else {
            // TODO: revm does not support pre-Bedrock blocks.
            // https://docs.optimism.io/builders/node-operators/architecture#legacy-geth
            Err(OptimismBlockSpecError::UnsupportedSpec)
        }
    }

    fn build_mv_memory(&self, block_env: &BlockEnv, txs: &[TxEnv]) -> MvMemory {
        let beneficiary_location_hash =
            hash_determinisitic(MemoryLocation::Basic(block_env.coinbase));
        let l1_fee_recipient_location_hash = hash_determinisitic(revm::L1_FEE_RECIPIENT);
        let base_fee_recipient_location_hash = hash_determinisitic(revm::BASE_FEE_RECIPIENT);

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        for (index, tx) in txs.iter().enumerate() {
            if tx.optimism.source_hash.is_none() {
                estimated_locations
                    .entry(beneficiary_location_hash)
                    .or_insert_with(|| Vec::with_capacity(txs.len()))
                    .push(index);
            } else {
                // TODO: Benchmark to check whether adding these estimated
                // locations helps or harms the performance.
                estimated_locations
                    .entry(l1_fee_recipient_location_hash)
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(index);
                estimated_locations
                    .entry(base_fee_recipient_location_hash)
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(index);
            }
        }

        MvMemory::new(
            txs.len(),
            estimated_locations,
            [
                block_env.coinbase,
                revm::L1_FEE_RECIPIENT,
                revm::BASE_FEE_RECIPIENT,
            ],
        )
    }

    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB> {
        Handler::optimism_with_spec(spec_id, with_reward_beneficiary)
    }

    fn get_reward_policy(&self) -> RewardPolicy {
        RewardPolicy::Optimism {
            l1_fee_recipient_location_hash: hash_determinisitic(MemoryLocation::Basic(
                revm::optimism::L1_FEE_RECIPIENT,
            )),
            base_fee_vault_location_hash: hash_determinisitic(MemoryLocation::Basic(
                revm::optimism::BASE_FEE_RECIPIENT,
            )),
        }
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    // https://github.com/paradigmxyz/reth/blob/b4a1b733c93f7e262f1b774722670e08cdcb6276/crates/primitives/src/proofs.rs
    fn calculate_receipt_root(
        &self,
        spec_id: SpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError> {
        // 1. Create a list of [ReceiptEnvelope]
        let receipt_envelopes: Vec<OpReceiptEnvelope> =
            Iterator::zip(txs.txns(), tx_results.iter())
                .map(|(tx, tx_result)| {
                    let receipt = tx_result.receipt.clone();
                    let byte = tx.inner.transaction_type.unwrap_or_default();
                    let tx_type = OpTxType::try_from(byte)
                        .map_err(|_| CalculateReceiptRootError::InvalidTxType(byte))?;
                    Ok(match tx_type {
                        OpTxType::Legacy => OpReceiptEnvelope::Legacy(receipt.with_bloom()),
                        OpTxType::Eip2930 => OpReceiptEnvelope::Eip2930(receipt.with_bloom()),
                        OpTxType::Eip1559 => OpReceiptEnvelope::Eip1559(receipt.with_bloom()),
                        OpTxType::Eip7702 => OpReceiptEnvelope::Eip7702(receipt.with_bloom()),
                        OpTxType::Deposit => {
                            let account_maybe = tx_result
                                .state
                                .get(&tx.inner.from)
                                .expect("Sender not found");
                            let account = account_maybe.as_ref().expect("Sender not changed");
                            let receipt = OpDepositReceipt {
                                inner: receipt,
                                deposit_nonce: (spec_id >= SpecId::CANYON)
                                    .then_some(account.nonce - 1),
                                deposit_receipt_version: (spec_id >= SpecId::CANYON).then_some(1),
                            };
                            OpReceiptEnvelope::Deposit(receipt.with_bloom())
                        }
                    })
                })
                .collect::<Result<_, _>>()?;

        // 2. Create a trie then calculate the root hash
        // We use [BTreeMap] because the keys must be sorted in ascending order.
        let trie_entries: BTreeMap<_, _> = receipt_envelopes
            .iter()
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

    fn get_tx_env(&self, tx: Self::Transaction) -> Result<TxEnv, OptimismTransactionParsingError> {
        Ok(TxEnv {
            optimism: get_optimism_fields(&tx)?,
            caller: tx.inner.from,
            gas_limit: tx.inner.gas,
            gas_price: get_optimism_gas_price(&tx)?,
            gas_priority_fee: tx.inner.max_priority_fee_per_gas.map(U256::from),
            transact_to: tx.inner.to.into(),
            value: tx.inner.value,
            data: tx.inner.input,
            nonce: Some(tx.inner.nonce),
            chain_id: tx.inner.chain_id,
            access_list: tx.inner.access_list.unwrap_or_default().into(),
            blob_hashes: tx.inner.blob_versioned_hashes.unwrap_or_default(),
            max_fee_per_blob_gas: tx.inner.max_fee_per_blob_gas.map(U256::from),
            authorization_list: tx.inner.authorization_list.map(AuthorizationList::Signed),
        })
    }

    fn is_eip_1559_enabled(&self, _spec_id: SpecId) -> bool {
        true
    }

    fn is_eip_161_enabled(&self, _spec_id: SpecId) -> bool {
        true
    }
}
