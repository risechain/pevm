use alloy_primitives::Bloom;
use alloy_rpc_types::Block;
use pevm::{
    chain::{CalculateReceiptRootError, PevmChain, PevmEthereum},
    EvmAccount, ParallelParams, Pevm, PevmStrategy, Storage,
};
use revm::primitives::{alloy_primitives::U160, Address, BlockEnv, SpecId, TxEnv, U256};

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
pub fn test_execute_revm<S: Storage + Send + Sync>(storage: S, txs: Vec<TxEnv>) {
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
            ParallelParams::default()
        ),
    );
}

// Execute an Alloy block sequentially & with pevm and assert that
// the execution results match.
pub fn test_execute_alloy<S: Storage + Send + Sync, C: PevmChain + Send + Sync + PartialEq>(
    storage: &S,
    chain: &C,
    block: Block<C::Transaction>,
    must_match_block_header: bool,
) {
    let mut pevm = Pevm::default();
    let sequential_result = pevm.execute(storage, chain, block.clone(), PevmStrategy::sequential());
    let parallel_result = pevm.execute(
        storage,
        chain,
        block.clone(),
        PevmStrategy::auto(block.transactions.len(), block.header.gas_used),
    );
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
