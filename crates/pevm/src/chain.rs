//! Chain specific utils

use std::error::Error as StdError;
use std::fmt::Debug;

use alloy_consensus::{Signed, TxLegacy, transaction::Recovered};
use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::{BlockTransactions, Header, Transaction};
use revm::{
    Handler,
    primitives::{BlockEnv, SpecId, TxEnv},
};

use crate::{PevmTxExecutionResult, mv_memory::MvMemory};

/// Different chains may have varying reward policies.
/// This enum specifies which policy to follow, with optional
/// pre-calculated data to assist in reward calculations.
#[derive(Debug, Clone)]
pub enum RewardPolicy {
    /// Ethereum
    Ethereum,
    /// Optimism
    #[cfg(feature = "optimism")]
    Optimism {
        /// L1 Fee Recipient
        l1_fee_recipient_location_hash: crate::MemoryLocationHash,
        /// Base Fee Vault
        base_fee_vault_location_hash: crate::MemoryLocationHash,
    },
}

/// The error type of [`PevmChain::calculate_receipt_root`]
#[derive(Debug, Clone)]
pub enum CalculateReceiptRootError {
    /// Unsupported
    Unsupported,
    /// Invalid transaction type
    InvalidTxType(u8),
    /// Arbitrary error message
    Custom(String),
    /// Optimism deposit is missing sender
    #[cfg(feature = "optimism")]
    OpDepositMissingSender,
}

/// Custom behaviours for different chains & networks
pub trait PevmChain: Debug {
    /// The network type
    type Network: alloy_provider::Network<BlockResponse: Into<alloy_rpc_types_eth::Block<Self::Transaction>>>;

    /// The transaction type
    type Transaction: Debug + Clone + PartialEq;

    /// The envelope type
    // TODO: Support more tx conversions
    type Envelope: Debug + From<Signed<TxLegacy>>;

    /// The error type for [`Self::get_block_spec`].
    type BlockSpecError: StdError + Debug + Clone + PartialEq + 'static;

    /// The error type for [`Self::get_tx_env`].
    type TransactionParsingError: StdError + Debug + Clone + PartialEq + 'static;

    /// Get chain id.
    fn id(&self) -> u64;

    /// Mock RPC transaction for testing.
    fn mock_rpc_tx(envelope: Self::Envelope, from: Address) -> Transaction<Self::Envelope> {
        Transaction {
            inner: Recovered::new_unchecked(envelope, from),
            block_hash: None,
            block_number: None,
            transaction_index: None,
            effective_gas_price: None,
        }
    }

    /// Mock `Self::Transaction` for testing.
    fn mock_tx(&self, envelope: Self::Envelope, from: Address) -> Self::Transaction;

    /// Get block's [`SpecId`]
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError>;

    /// Get [`TxEnv`]
    fn get_tx_env(&self, tx: &Self::Transaction) -> Result<TxEnv, Self::TransactionParsingError>;

    /// Build [`MvMemory`]
    fn build_mv_memory(&self, _block_env: &BlockEnv, txs: &[TxEnv]) -> MvMemory {
        MvMemory::new(txs.len(), [], [])
    }

    /// Get [Handler]
    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB>;

    /// Get [`RewardPolicy`]
    fn get_reward_policy(&self) -> RewardPolicy;

    /// Calculate receipt root
    fn calculate_receipt_root(
        &self,
        spec_id: SpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError>;

    /// Check whether EIP-1559 is enabled
    /// <https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-1559.md>
    fn is_eip_1559_enabled(&self, spec_id: SpecId) -> bool;

    /// Check whether EIP-161 is enabled
    /// <https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-161.md>
    fn is_eip_161_enabled(&self, spec_id: SpecId) -> bool;
}

mod ethereum;
pub use ethereum::PevmEthereum;

#[cfg(feature = "optimism")]
mod optimism;
#[cfg(feature = "optimism")]
pub use optimism::PevmOptimism;
