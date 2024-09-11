//! Chain specific utils

use std::fmt::Debug;

use alloy_primitives::B256;
use alloy_rpc_types::{BlockTransactions, Header};
use revm::{
    primitives::{BlockEnv, SpecId, TxEnv},
    Handler,
};

use crate::{mv_memory::MvMemory, PevmTxExecutionResult};

/// Different chains may have varying reward policies.
/// This enum specifies which policy to follow, with optional
/// pre-calculated data to assist in reward calculations.
#[derive(Debug, Clone)]
pub enum RewardPolicy {
    /// Ethereum
    Ethereum,
}

/// The error type of [PevmChain::calculate_receipt_root]
#[derive(Debug, Clone)]
pub enum CalculateReceiptRootError {
    /// Unsupported
    Unsupported,
    /// Invalid transaction type
    InvalidTxType(u8),
    /// Arbitrary error message
    Custom(String),
}

/// Custom behaviours for different chains & networks
pub trait PevmChain: Debug {
    /// The transaction type
    type Transaction: Debug + Clone + PartialEq;

    /// The error type for [Self::get_block_spec].
    type BlockSpecError: Debug + Clone + PartialEq;

    /// The error type for [Self::get_tx_env].
    type TransactionParsingError: Debug + Clone + PartialEq;

    /// Get chain id.
    fn id(&self) -> u64;

    /// Build Self::Transaction type from Alloy's transaction
    fn build_tx_from_alloy_tx(&self, tx: alloy_rpc_types::Transaction) -> Self::Transaction;

    /// Get block's [SpecId]
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError>;

    /// Get [TxEnv]
    fn get_tx_env(&self, tx: Self::Transaction) -> Result<TxEnv, Self::TransactionParsingError>;

    /// Build [MvMemory]
    fn build_mv_memory(
        &self,
        _hasher: &ahash::RandomState,
        _block_env: &BlockEnv,
        txs: &[TxEnv],
    ) -> MvMemory {
        MvMemory::new(txs.len(), [], [])
    }

    /// Get [Handler]
    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB>;

    /// Get [RewardPolicy]
    fn get_reward_policy(&self, hasher: &ahash::RandomState) -> RewardPolicy;

    /// Calculate receipt root
    fn calculate_receipt_root(
        &self,
        spec_id: SpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError>;

    /// Check whether EIP-1559 is enabled
    /// https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-1559.md
    fn is_eip_1559_enabled(&self, spec_id: SpecId) -> bool;

    /// Check whether EIP-161 is enabled
    /// https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-161.md
    fn is_eip_161_enabled(&self, spec_id: SpecId) -> bool;
}

mod ethereum;
pub use ethereum::PevmEthereum;
