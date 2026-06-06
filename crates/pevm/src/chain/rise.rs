//! RISE
use alloy_consensus::Transaction;
use alloy_primitives::{Address, B256, ChainId, U256};
use alloy_rpc_types_eth::{BlockTransactions, Header};
use hashbrown::HashMap;
use op_alloy_consensus::{OpDepositReceipt, OpReceiptEnvelope, OpTxEnvelope, OpTxType};
use op_alloy_network::eip2718::Encodable2718;
use op_revm::{
    L1BlockInfo, OpBuilder, OpContext, OpEvm, OpHaltReason, OpSpecId, OpTransaction,
    OpTransactionError,
    constants::{BASE_FEE_RECIPIENT, L1_FEE_RECIPIENT, OPERATOR_FEE_RECIPIENT},
    transaction::{OpTxTr, deposit::DepositTransactionParts},
};
use revm::{
    Context, Database, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::either::Either,
    handler::EvmTr,
};
use smallvec::SmallVec;

use crate::{
    BuildIdentityHasher, LocationTag, MemoryLocation, MemoryLocationHash, PevmTxExecutionResult,
    hash_deterministic, mv_memory::MvMemory, tag_deterministic,
};

use super::{CalculateReceiptRootError, PevmChain};

const RISE_CHAIN_ID: ChainId = 4153; // Mainnet

// Compute the (hash, tag) pair of a basic-account location under the per-block seed.
// The fee recipients are fixed addresses, but a per-execution seed means their
// fingerprints cannot be precomputed once, so we derive them per block here.
#[inline]
fn basic_hash_and_tag(address: Address, seed: u64) -> (MemoryLocationHash, LocationTag) {
    (
        hash_deterministic(MemoryLocation::Basic(address), seed),
        tag_deterministic(MemoryLocation::Basic(address), seed),
    )
}

/// Implementation of [`PevmChain`] for RISE
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PevmRise;

/// Represents errors that can occur when parsing RISE transactions
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RiseTransactionParsingError {
    /// Transaction is missing a gas price
    #[error("Transaction must set gas price")]
    MissingGasPrice,
}

fn get_gas_price(tx: &OpTxEnvelope) -> Result<u128, RiseTransactionParsingError> {
    match tx.tx_type() {
        OpTxType::Legacy | OpTxType::Eip2930 => tx
            .gas_price()
            .ok_or(RiseTransactionParsingError::MissingGasPrice),
        OpTxType::Eip1559 | OpTxType::Eip7702 => Ok(tx.max_fee_per_gas()),
        OpTxType::Deposit | OpTxType::PostExec => Ok(0),
    }
}

impl PevmChain for PevmRise {
    type Network = op_alloy_network::Optimism;
    type Transaction = op_alloy_rpc_types::Transaction;
    type Envelope = OpTxEnvelope;
    type Evm<DB: Database> = OpEvm<OpContext<DB>, ()>;
    type EvmSpecId = OpSpecId;
    type EvmTx = OpTransaction<TxEnv>;
    type EvmHaltReason = OpHaltReason;
    type EvmErrorType = OpTransactionError;
    type BlockSpecError = std::convert::Infallible;
    type TransactionParsingError = RiseTransactionParsingError;

    fn id(&self) -> ChainId {
        RISE_CHAIN_ID
    }

    fn mock_tx(&self, envelope: Self::Envelope, from: Address) -> Self::Transaction {
        Self::Transaction {
            inner: Self::mock_rpc_tx(envelope, from),
            deposit_nonce: None,
            deposit_receipt_version: None,
        }
    }

    fn get_block_spec(&self, _header: &Header) -> Result<OpSpecId, Self::BlockSpecError> {
        // RISE Mainnet launched as JOVIAN; currently all blocks use this spec.
        Ok(OpSpecId::JOVIAN)
    }

    fn build_evm<DB: Database>(
        &self,
        spec_id: Self::EvmSpecId,
        block_env: BlockEnv,
        db: DB,
    ) -> Self::Evm<DB> {
        Context::mainnet()
            .with_cfg(CfgEnv::new_with_spec(spec_id).with_chain_id(RISE_CHAIN_ID))
            .with_block(block_env)
            .with_db(db)
            .with_tx(OpTransaction::default())
            .with_chain(L1BlockInfo::default())
            .build_op()
    }

    fn build_mv_memory(
        &self,
        block_env: &BlockEnv,
        txs: &[OpTransaction<TxEnv>],
        seed: u64,
    ) -> MvMemory {
        // The beneficiary and the three fee recipients are written by every
        // non-deposit transaction, so we pre-seed them as estimates.
        let recipients = [
            block_env.beneficiary,
            BASE_FEE_RECIPIENT,
            L1_FEE_RECIPIENT,
            OPERATOR_FEE_RECIPIENT,
        ];

        // TODO: Estimate more locations based on sender, to, etc.
        // TODO: Benchmark to check whether adding these estimated
        // locations helps or harms the performance.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        for (index, tx) in txs.iter().enumerate() {
            // Deposit transactions pay no fees so they write nothing to these fee recipients.
            if !tx.is_deposit() {
                for address in recipients {
                    let (hash, tag) = basic_hash_and_tag(address, seed);
                    estimated_locations
                        .entry(hash)
                        .or_insert_with(|| (tag, Vec::with_capacity(txs.len())))
                        .1
                        .push(index);
                }
            }
        }

        MvMemory::new(
            seed,
            txs.len(),
            estimated_locations
                .into_iter()
                .map(|(hash, (tag, tx_idxs))| (hash, tag, tx_idxs)),
            recipients,
        )
    }

    fn get_rewards(
        &self,
        beneficiary: (MemoryLocationHash, LocationTag),
        gas_used: U256,
        gas_price: U256,
        basefee: u64,
        seed: u64,
        tx: &Self::EvmTx,
    ) -> SmallVec<[(MemoryLocationHash, LocationTag, U256); 1]> {
        if tx.is_deposit() {
            SmallVec::new()
        } else {
            let (base_fee_hash, base_fee_tag) = basic_hash_and_tag(BASE_FEE_RECIPIENT, seed);
            let (l1_fee_hash, l1_fee_tag) = basic_hash_and_tag(L1_FEE_RECIPIENT, seed);
            let (operator_fee_hash, operator_fee_tag) =
                basic_hash_and_tag(OPERATOR_FEE_RECIPIENT, seed);
            smallvec::smallvec![
                (
                    beneficiary.0,
                    beneficiary.1,
                    gas_price.saturating_mul(gas_used)
                ),
                (
                    base_fee_hash,
                    base_fee_tag,
                    U256::from(basefee).saturating_mul(gas_used),
                ),
                // RISE disables DA footprint and operator fees. Annoyingly, we still
                // need to touch these to match revm's sequential execution for now.
                // Will remove once we rewrite our own EVM implementation.
                (l1_fee_hash, l1_fee_tag, U256::ZERO),
                (operator_fee_hash, operator_fee_tag, U256::ZERO),
            ]
        }
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    // https://github.com/paradigmxyz/reth/blob/b4a1b733c93f7e262f1b774722670e08cdcb6276/crates/primitives/src/proofs.rs
    fn calculate_receipt_root(
        &self,
        _: OpSpecId,
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
                            deposit_nonce: Some(account.nonce - 1),
                            deposit_receipt_version: Some(1),
                        };
                        OpReceiptEnvelope::Deposit(receipt.with_bloom())
                    }
                    OpTxType::PostExec => OpReceiptEnvelope::PostExec(receipt.with_bloom()),
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
    ) -> Result<OpTransaction<TxEnv>, RiseTransactionParsingError> {
        Ok(OpTransaction {
            base: TxEnv {
                tx_type: tx.inner.inner.tx_type().into(),
                caller: tx.inner.inner.signer(),
                gas_limit: tx.gas_limit(),
                gas_price: get_gas_price(&tx.inner.inner)?,
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
                    .map(|auths| auths.iter().cloned().map(Either::Left).collect())
                    .unwrap_or_default(),
            },
            enveloped_tx: if tx.inner.inner.is_deposit() {
                None
            } else {
                Some(tx.inner.inner.encoded_2718().into())
            },
            deposit: if let Some(deposit) = tx.inner.inner.as_deposit() {
                DepositTransactionParts::new(
                    deposit.source_hash,
                    Some(deposit.mint),
                    deposit.is_system_transaction,
                )
            } else {
                DepositTransactionParts::new(B256::ZERO, None, false)
            },
        })
    }

    fn tx_env<'a>(&self, tx: &'a OpTransaction<TxEnv>) -> &'a TxEnv {
        &tx.base
    }

    fn has_nonce<DB: Database>(&self, evm: &mut Self::Evm<DB>, tx: &Self::EvmTx) -> bool {
        let is_deposit = tx.is_deposit();
        evm.ctx()
            .modify_cfg(|cfg| cfg.disable_nonce_check = is_deposit);
        !is_deposit
    }

    fn is_eip_1559_enabled(&self, _: OpSpecId) -> bool {
        true
    }

    fn is_eip_161_enabled(&self, _: OpSpecId) -> bool {
        true
    }
}
