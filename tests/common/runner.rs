use alloy_primitives::Bloom;
use alloy_rpc_types::Block;
use pevm::{
    chain::{PevmChain, PevmEthereum},
    EvmAccount, Pevm, PevmTxExecutionResult,
};
use revm::{
    primitives::{alloy_primitives::U160, Address, BlockEnv, SpecId, TxEnv, U256},
    DatabaseRef,
};
use std::{fmt::Display, num::NonZeroUsize, thread};

// Mock an account from an integer index that is used as the address.
// Useful for mock iterations.
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

// Execute an REVM block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_revm<S: DatabaseRef<Error: Display> + Send + Sync>(
    storage: S,
    txs: Vec<TxEnv>,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_eq!(
        pevm::execute_revm_sequential(
            &storage,
            &PevmEthereum::mainnet(),
            SpecId::LATEST,
            BlockEnv::default(),
            txs.clone(),
        ),
        Pevm::default().execute_revm_parallel(
            &storage,
            &PevmEthereum::mainnet(),
            SpecId::LATEST,
            BlockEnv::default(),
            txs,
            concurrency_level,
        ),
    );
}

// Execute an Alloy block sequentially & with pevm and assert that
// the execution results match.
pub fn test_execute_alloy<
    S: DatabaseRef<Error: Display> + Send + Sync,
    C: PevmChain + Send + Sync + PartialEq,
>(
    storage: &S,
    chain: &C,
    block: Block<C::Transaction>,
    must_match_block_header: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let mut pevm = Pevm::default();
    let sequential_result = pevm.execute(storage, chain, block.clone(), concurrency_level, true);
    let parallel_result = pevm.execute(storage, chain, block.clone(), concurrency_level, false);
    assert_eq!(&sequential_result, &parallel_result);

    let spec_id = chain.get_block_spec(&block.header).unwrap();
    let mut cumulative_gas_used: u128 = 0;
    let tx_results = sequential_result
        .unwrap()
        .into_iter()
        .map(|result_and_state| {
            let mut result = PevmTxExecutionResult::from_revm(spec_id, result_and_state);
            result.receipt.cumulative_gas_used += cumulative_gas_used;
            cumulative_gas_used = result.receipt.cumulative_gas_used;
            result
        })
        .collect::<Vec<_>>();
    if must_match_block_header {
        // We can only calculate the receipts root from Byzantium.
        // Before EIP-658 (https://eips.ethereum.org/EIPS/eip-658), the
        // receipt root is calculated with the post transaction state root,
        // which we don't have in these tests.
        if block.header.number >= 4370000 {
            assert_eq!(
                block.header.receipts_root,
                chain.calculate_receipt_root(spec_id, &block.transactions, &tx_results)
            );
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
