//! Optimism
use alloy_consensus::Transaction;
use alloy_primitives::{Address, B256, ChainId, U256};
use alloy_rpc_types_eth::{BlockTransactions, Header};
use hashbrown::HashMap;
use op_alloy_consensus::{OpDepositReceipt, OpReceiptEnvelope, OpTxEnvelope, OpTxType};
use op_alloy_network::eip2718::Encodable2718;
use op_revm::{
    L1BlockInfo, OpBuilder, OpContext, OpEvm, OpHaltReason, OpSpecId, OpTransaction,
    OpTransactionError,
    constants::{BASE_FEE_RECIPIENT, L1_FEE_RECIPIENT},
    transaction::{OpTxTr, deposit::DepositTransactionParts},
};
use revm::{
    Context, Database, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv},
};
use smallvec::SmallVec;

use crate::{
    BuildIdentityHasher, MemoryLocation, MemoryLocationHash, PevmTxExecutionResult,
    hash_deterministic, mv_memory::MvMemory,
};

use super::{CalculateReceiptRootError, PevmChain};

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptimismBlockSpecError {
    #[error("Spec is not supported")]
    UnsupportedSpec,
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptimismTransactionParsingError {
    #[error("Transaction must set gas price")]
    MissingGasPrice,
}

fn get_optimism_gas_price(tx: &OpTxEnvelope) -> Result<u128, OptimismTransactionParsingError> {
    match tx.tx_type() {
        OpTxType::Legacy | OpTxType::Eip2930 => tx
            .gas_price()
            .ok_or(OptimismTransactionParsingError::MissingGasPrice),
        OpTxType::Eip1559 | OpTxType::Eip7702 => Ok(tx.max_fee_per_gas()),
        OpTxType::Deposit => Ok(0),
    }
}

impl PevmChain for PevmOptimism {
    type Network = op_alloy_network::Optimism;
    type Transaction = op_alloy_rpc_types::Transaction;
    type Envelope = OpTxEnvelope;
    type Evm<DB: Database> = OpEvm<OpContext<DB>, ()>;
    type EvmSpecId = OpSpecId;
    type EvmTx = OpTransaction<TxEnv>;
    type EvmHaltReason = OpHaltReason;
    type EvmErrorType = OpTransactionError;
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

    fn get_block_spec(&self, header: &Header) -> Result<OpSpecId, Self::BlockSpecError> {
        // TODO: The implementation below is only true for Optimism Mainnet.
        // When supporting other networks (e.g. Optimism Sepolia), remember to adjust the code here.
        if header.timestamp >= 1720627201 {
            Ok(OpSpecId::FJORD)
        } else if header.timestamp >= 1710374401 {
            Ok(OpSpecId::ECOTONE)
        } else if header.timestamp >= 1704992401 {
            Ok(OpSpecId::CANYON)
        } else if header.number >= 105235063 {
            // On Optimism Mainnet, Bedrock and Regolith are activated at the same time.
            // Therefore, this function never returns OpSpecId::BEDROCK.
            // The statement above might not be true for other networks, e.g. Optimism Goerli.
            Ok(OpSpecId::REGOLITH)
        } else {
            // TODO: revm does not support pre-Bedrock blocks.
            // https://docs.optimism.io/builders/node-operators/architecture#legacy-geth
            Err(OptimismBlockSpecError::UnsupportedSpec)
        }
    }

    fn build_evm<DB: Database>(
        &self,
        spec_id: Self::EvmSpecId,
        block_env: BlockEnv,
        db: DB,
    ) -> Self::Evm<DB> {
        Context::mainnet()
            .with_cfg(CfgEnv::new_with_spec(spec_id))
            .with_block(block_env)
            .with_db(db)
            .with_tx(OpTransaction::default())
            .with_chain(L1BlockInfo::default())
            .build_op()
    }

    fn build_mv_memory(&self, block_env: &BlockEnv, txs: &[OpTransaction<TxEnv>]) -> MvMemory {
        let beneficiary_location_hash =
            hash_deterministic(MemoryLocation::Basic(block_env.beneficiary));
        let l1_fee_recipient_location_hash = hash_deterministic(L1_FEE_RECIPIENT);
        let base_fee_recipient_location_hash = hash_deterministic(BASE_FEE_RECIPIENT);

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        for (index, tx) in txs.iter().enumerate() {
            if tx.is_deposit() {
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
            [block_env.beneficiary, L1_FEE_RECIPIENT, BASE_FEE_RECIPIENT],
        )
    }

    fn get_rewards<DB: Database>(
        &self,
        beneficiary_location_hash: u64,
        gas_used: U256,
        gas_price: U256,
        evm: &mut Self::Evm<DB>,
        tx: &Self::EvmTx,
    ) -> SmallVec<[(MemoryLocationHash, U256); 1]> {
        let is_deposit = !tx.deposit.source_hash.is_empty();
        if is_deposit {
            SmallVec::new()
        } else {
            let Some(enveloped_tx) = &tx.enveloped_tx else {
                panic!("[OPTIMISM] Failed to load enveloped transaction.");
            };
            let spec_id = evm.0.cfg.spec;
            let l1_cost = evm.0.chain.calculate_tx_l1_cost(enveloped_tx, spec_id);
            let l1_fee_recipient_location_hash = hash_deterministic(L1_FEE_RECIPIENT);
            let base_fee_recipient_location_hash = hash_deterministic(BASE_FEE_RECIPIENT);
            smallvec::smallvec![
                (
                    beneficiary_location_hash,
                    gas_price.saturating_mul(gas_used)
                ),
                (l1_fee_recipient_location_hash, l1_cost),
                (
                    base_fee_recipient_location_hash,
                    U256::from(evm.0.block.basefee).saturating_mul(gas_used),
                ),
            ]
        }
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    // https://github.com/paradigmxyz/reth/blob/b4a1b733c93f7e262f1b774722670e08cdcb6276/crates/primitives/src/proofs.rs
    fn calculate_receipt_root(
        &self,
        spec_id: OpSpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError> {
        let mut trie_entries = txs
            .txns()
            .zip(tx_results.iter())
            .map(|(tx, tx_result)| {
                let receipt = tx_result.receipt.clone();
                Ok(match tx.inner.inner.tx_type() {
                    OpTxType::Legacy => OpReceiptEnvelope::Legacy(receipt.with_bloom()),
                    OpTxType::Eip2930 => OpReceiptEnvelope::Eip2930(receipt.with_bloom()),
                    OpTxType::Eip1559 => OpReceiptEnvelope::Eip1559(receipt.with_bloom()),
                    OpTxType::Eip7702 => OpReceiptEnvelope::Eip7702(receipt.with_bloom()),
                    OpTxType::Deposit => {
                        let account = tx_result
                            .state
                            .get(tx.inner.inner.signer_ref())
                            .and_then(Option::as_ref)
                            .ok_or(CalculateReceiptRootError::OpDepositMissingSender)?;
                        let receipt = OpDepositReceipt {
                            inner: receipt,
                            deposit_nonce: (spec_id >= OpSpecId::CANYON)
                                .then_some(account.nonce - 1),
                            deposit_receipt_version: (spec_id >= OpSpecId::CANYON).then_some(1),
                        };
                        OpReceiptEnvelope::Deposit(receipt.with_bloom())
                    }
                })
            })
            .enumerate()
            .map(|(index, receipt)| {
                Ok((
                    alloy_rlp::encode_fixed_size(&index),
                    receipt?.encoded_2718(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        trie_entries.sort();

        let mut hash_builder = alloy_trie::HashBuilder::default();
        for (k, v) in trie_entries {
            hash_builder.add_leaf(alloy_trie::Nibbles::unpack(&k), &v);
        }
        Ok(hash_builder.root())
    }

    fn get_tx_env(
        &self,
        tx: &Self::Transaction,
    ) -> Result<OpTransaction<TxEnv>, OptimismTransactionParsingError> {
        Ok(OpTransaction {
            base: TxEnv {
                tx_type: tx.inner.inner.tx_type().into(),
                caller: tx.inner.inner.signer(),
                gas_limit: tx.gas_limit(),
                gas_price: get_optimism_gas_price(&tx.inner.inner)?,
                gas_priority_fee: tx.max_priority_fee_per_gas(),
                kind: tx.kind(),
                value: tx.value(),
                data: tx.input().clone(),
                nonce: tx.nonce(),
                chain_id: tx.chain_id(),
                access_list: tx.access_list().cloned().unwrap_or_default(),
                blob_hashes: tx.blob_versioned_hashes().unwrap_or_default().to_vec(),
                max_fee_per_blob_gas: tx.max_fee_per_blob_gas().unwrap_or_default(),
                authorization_list: tx
                    .authorization_list()
                    .map(|auths| auths.to_vec())
                    .unwrap_or_default(),
            },
            enveloped_tx: None, // TODO: Do we need to fill this?
            deposit: if let Some(deposit) = tx.inner.inner.as_deposit() {
                DepositTransactionParts::new(
                    deposit.source_hash,
                    deposit.mint,
                    deposit.is_system_transaction,
                )
            } else {
                DepositTransactionParts::new(B256::ZERO, None, false)
            },
        })
    }

    fn into_tx_env(&self, tx: Self::EvmTx) -> TxEnv {
        tx.base
    }

    fn is_eip_1559_enabled(&self, _: OpSpecId) -> bool {
        true
    }

    fn is_eip_161_enabled(&self, _: OpSpecId) -> bool {
        true
    }
}
