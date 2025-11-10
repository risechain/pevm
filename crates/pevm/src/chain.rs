//! Chain specific utils

use std::fmt::Debug;
use std::{error::Error as StdError, fmt::Display};

use alloy_consensus::{Signed, TxLegacy, transaction::Recovered};
use alloy_primitives::{Address, B256, U256};
use alloy_rpc_types_eth::{BlockTransactions, Header, Transaction};
use revm::context::result::HaltReason;
use revm::context::{JournalOutput, JournalTr, TxEnv};
use revm::handler::PrecompileProvider;
use revm::handler::instructions::InstructionProvider;
use revm::interpreter::InterpreterResult;
use revm::interpreter::interpreter::EthInterpreter;
use revm::primitives::hardfork::SpecId;
use revm::{
    Database, ExecuteEvm,
    context::{
        BlockEnv, ContextTr,
        result::{EVMError, ResultAndState},
    },
    handler::EvmTr,
};
use smallvec::SmallVec;

use crate::{MemoryLocationHash, PevmTxExecutionResult, mv_memory::MvMemory};

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

    /// The EVM type
    type Evm<DB: Database>: EvmTr<
            Context: ContextTr<Db = DB, Journal: JournalTr<FinalOutput = JournalOutput>>,
            Precompiles: PrecompileProvider<
                <Self::Evm<DB> as EvmTr>::Context,
                Output = InterpreterResult,
            >,
            Instructions: InstructionProvider<
                Context = <Self::Evm<DB> as EvmTr>::Context,
                InterpreterTypes = EthInterpreter,
            >,
        > + ExecuteEvm<
            Tx = Self::EvmTx,
            Output = Result<
                ResultAndState<Self::EvmHaltReason>,
                EVMError<DB::Error, Self::EvmErrorType>,
            >,
        >;

    /// The EVM Spec type
    type EvmSpecId: Into<SpecId> + Copy + Send + Sync + Default;

    /// The EVM Tx type
    type EvmTx: Clone + Send + Sync;

    /// The EVM halt reason
    type EvmHaltReason: From<HaltReason> + Eq + Debug + Clone;

    /// The EVM error type
    type EvmErrorType: Display;

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

    /// Get block's spec id
    fn get_block_spec(&self, header: &Header) -> Result<Self::EvmSpecId, Self::BlockSpecError>;

    /// Get `Self::Evm`
    fn build_evm<DB: Database>(
        &self,
        spec_id: Self::EvmSpecId,
        block_env: BlockEnv,
        db: DB,
    ) -> Self::Evm<DB>;

    /// Get `Self::EvmTx`
    fn get_tx_env(
        &self,
        tx: &Self::Transaction,
    ) -> Result<Self::EvmTx, Self::TransactionParsingError>;

    ///
    fn into_tx_env(&self, tx: Self::EvmTx) -> TxEnv;

    /// Build [`MvMemory`]
    fn build_mv_memory(&self, _block_env: &BlockEnv, txs: &[Self::EvmTx]) -> MvMemory {
        MvMemory::new(txs.len(), [], [])
    }

    /// Get rewards (balance increments) to beneficiary accounts, etc.
    fn get_rewards<DB: Database>(
        &self,
        beneficiary_location_hash: u64,
        gas_used: U256,
        gas_price: U256,
        evm: &mut Self::Evm<DB>,
        tx: &Self::EvmTx,
    ) -> SmallVec<[(MemoryLocationHash, U256); 1]>;

    /// Calculate receipt root
    fn calculate_receipt_root(
        &self,
        spec_id: Self::EvmSpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError>;

    /// Check whether EIP-1559 is enabled
    /// <https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-1559.md>
    fn is_eip_1559_enabled(&self, spec_id: Self::EvmSpecId) -> bool;

    /// Check whether EIP-161 is enabled
    /// <https://github.com/ethereum/EIPs/blob/96523ef4d76ca440f73f0403ddb5c9cb3b24dcae/EIPS/eip-161.md>
    fn is_eip_161_enabled(&self, spec_id: Self::EvmSpecId) -> bool;
}

mod ethereum;
pub use ethereum::PevmEthereum;

mod optimism;
pub use optimism::PevmOptimism;
