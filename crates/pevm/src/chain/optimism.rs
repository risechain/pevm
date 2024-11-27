//! Optimism
use std::collections::BTreeMap;

use alloy_consensus::Transaction;
use alloy_primitives::{Address, Bytes, ChainId, B256, U256};
use alloy_rpc_types_eth::{BlockTransactions, Header};
use hashbrown::HashMap;
use op_alloy_consensus::{
    DepositTransaction, OpDepositReceipt, OpReceiptEnvelope, OpTxEnvelope, OpTxType,
};
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

/// Implementation of [`PevmChain`] for Optimism
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PevmOptimism {
    id: ChainId,
}

impl PevmOptimism {
    /// Optimism Mainnet
    pub const fn mainnet() -> Self {
        Self { id: 10 }
    }

    /// Custom network
    pub const fn custom(id: ChainId) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimismBlockSpecError {
    UnsupportedSpec,
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimismTransactionParsingError {
    MissingGasPrice,
    MissingSourceHash,
}

fn get_optimism_gas_price(tx: &OpTxEnvelope) -> Result<U256, OptimismTransactionParsingError> {
    match tx.tx_type() {
        OpTxType::Legacy | OpTxType::Eip2930 => tx
            .gas_price()
            .map(U256::from)
            .ok_or(OptimismTransactionParsingError::MissingGasPrice),
        OpTxType::Eip1559 | OpTxType::Eip7702 => Ok(U256::from(tx.max_fee_per_gas())),
        OpTxType::Deposit => Ok(U256::ZERO),
    }
}

/// Extract [`OptimismFields`] from [`OpTxEnvelope`]
fn get_optimism_fields(
    tx: &OpTxEnvelope,
) -> Result<OptimismFields, OptimismTransactionParsingError> {
    let mut envelope_buf = Vec::<u8>::new();
    tx.encode_2718(&mut envelope_buf);

    let (source_hash, mint) = match &tx {
        OpTxEnvelope::Deposit(deposit) => (deposit.inner().source_hash(), deposit.inner().mint()),
        _ => (None, None),
    };

    Ok(OptimismFields {
        source_hash,
        mint,
        is_system_transaction: Some(tx.is_system_transaction()),
        enveloped_tx: Some(Bytes::from(envelope_buf)),
    })
}

impl PevmChain for PevmOptimism {
    type Transaction = op_alloy_rpc_types::Transaction;
    type Envelope = OpTxEnvelope;
    type BlockSpecError = OptimismBlockSpecError;
    type TransactionParsingError = OptimismTransactionParsingError;

    fn id(&self) -> ChainId {
        self.id
    }

    // TODO: allow to construct deposit transactions
    fn mock_tx(&self, envelope: Self::Envelope, from: Address) -> Self::Transaction {
        Self::Transaction {
            inner: Self::mock_rpc_tx(envelope, from),
            deposit_nonce: None,
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
                    Ok(match tx.inner.inner.tx_type() {
                        OpTxType::Legacy => OpReceiptEnvelope::Legacy(receipt.with_bloom()),
                        OpTxType::Eip2930 => OpReceiptEnvelope::Eip2930(receipt.with_bloom()),
                        OpTxType::Eip1559 => OpReceiptEnvelope::Eip1559(receipt.with_bloom()),
                        OpTxType::Eip7702 => OpReceiptEnvelope::Eip7702(receipt.with_bloom()),
                        OpTxType::Deposit => {
                            // TODO: Return proper errors instead of panic-ing.
                            let account_maybe =
                                tx_result.state.get(&tx.from).expect("Sender not found");
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

    fn get_tx_env(&self, tx: &Self::Transaction) -> Result<TxEnv, OptimismTransactionParsingError> {
        Ok(TxEnv {
            optimism: get_optimism_fields(&tx.inner.inner)?,
            caller: tx.from,
            gas_limit: tx.gas_limit(),
            gas_price: get_optimism_gas_price(&tx.inner.inner)?,
            gas_priority_fee: tx.max_priority_fee_per_gas().map(U256::from),
            transact_to: tx.kind(),
            value: tx.value(),
            data: tx.input().clone(),
            nonce: Some(tx.nonce()),
            chain_id: tx.chain_id(),
            access_list: tx.access_list().cloned().unwrap_or_default().to_vec(),
            blob_hashes: tx.blob_versioned_hashes().unwrap_or_default().to_vec(),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas().map(U256::from),
            authorization_list: tx
                .authorization_list()
                .map(|auths| AuthorizationList::Signed(auths.to_vec())),
        })
    }

    fn is_eip_1559_enabled(&self, _spec_id: SpecId) -> bool {
        true
    }

    fn is_eip_161_enabled(&self, _spec_id: SpecId) -> bool {
        true
    }
}
