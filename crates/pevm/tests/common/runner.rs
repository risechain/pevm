use alloy_primitives::Bloom;
use alloy_rpc_types_eth::Block;
use op_revm::OpTransaction;
use pevm::{
    EvmAccount, Pevm, Storage,
    chain::{CalculateReceiptRootError, PevmChain, PevmEthereum, PevmRise},
};
use revm::{
    context::{BlockEnv, TxEnv},
    primitives::{Address, U256, alloy_primitives::U160},
};
use std::{fmt::Debug, num::NonZeroUsize, thread};

/// Chains whose EVM transactions can be mocked from a base [`TxEnv`],
/// so the same mocked blocks can be tested on every chain.
pub trait MockEvmTx: PevmChain {
    /// Wrap a base [`TxEnv`] into this chain's EVM transaction.
    fn mock_evm_tx(&self, tx: TxEnv) -> Self::EvmTx;
}

impl MockEvmTx for PevmEthereum {
    fn mock_evm_tx(&self, tx: TxEnv) -> TxEnv {
        tx
    }
}

impl MockEvmTx for PevmRise {
    fn mock_evm_tx(&self, base: TxEnv) -> OpTransaction<TxEnv> {
        // The default carries a mock enveloped tx, which non-deposit OP transactions require.
        OpTransaction {
            base: TxEnv {
                chain_id: Some(self.id()),
                ..base
            },
            ..OpTransaction::default()
        }
    }
}

/// Mock an account from an integer index that is used as the address.
/// Useful for mock iterations.
pub fn mock_account(idx: usize) -> (Address, EvmAccount) {
    let address = Address::from(U160::from(idx));
    let account = EvmAccount {
        // Filling half full accounts to have enough tokens for tests without worrying about
        // the corner case of balance not going beyond [U256::MAX].
        balance: U256::MAX.div_ceil(U256::from(2)),
        nonce: 1,
        ..EvmAccount::default()
    };
    (address, account)
}

/// Execute an REVM block sequentially and parallelly with PEVM and assert that
/// the execution results match.
pub fn test_execute_revm<C, S>(chain: &C, storage: S, txs: Vec<C::EvmTx>)
where
    C: PevmChain + PartialEq + Send + Sync,
    S: Storage + Send + Sync + Debug,
{
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_eq!(
        pevm::execute_revm_sequential(
            chain,
            &storage,
            C::EvmSpecId::default(),
            BlockEnv::default(),
            txs.clone(),
        ),
        Pevm::default().execute_revm_parallel(
            chain,
            &storage,
            C::EvmSpecId::default(),
            BlockEnv::default(),
            txs,
            concurrency_level,
        ),
    );
}

/// Execute an Alloy block sequentially & with pevm and assert that
/// the execution results match.
pub fn test_execute_alloy<C, S>(
    chain: &C,
    storage: &S,
    block: Block<C::Transaction>,
    must_match_block_header: bool,
) where
    C: PevmChain + PartialEq + Send + Sync,
    S: Storage + Send + Sync + Debug,
{
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let mut pevm = Pevm::default();
    let sequential_result = pevm.execute(chain, storage, &block, concurrency_level, true);
    let parallel_result = pevm.execute(chain, storage, &block, concurrency_level, false);
    assert!(sequential_result.is_ok());
    assert_eq!(&sequential_result, &parallel_result);

    let tx_results = sequential_result.unwrap();
    if must_match_block_header {
        let spec_id = chain.get_block_spec(&block.header).unwrap();

        match chain.calculate_receipt_root(spec_id, &block.transactions, &tx_results) {
            Ok(receipt_root) => assert_eq!(block.header.receipts_root, receipt_root),
            Err(CalculateReceiptRootError::Unsupported) => {}
            Err(err) => panic!("{:?}", err),
        }

        assert_eq!(
            block.header.logs_bloom,
            tx_results
                .iter()
                .map(|tx| tx.receipt.bloom_slow())
                .fold(Bloom::default(), |acc, bloom| acc.bit_or(bloom))
        );

        assert_eq!(
            block.header.gas_used,
            tx_results
                .iter()
                .last()
                .map(|result| result.receipt.cumulative_gas_used)
                .unwrap_or_default()
        );
    }
}
