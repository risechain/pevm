use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::{Bloom, B256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{Block, BlockTransactions, Transaction};
use pevm::{PevmResult, PevmTxExecutionResult, Storage};
use revm::{
    db::PlainAccount,
    primitives::{alloy_primitives::U160, AccountInfo, Address, BlockEnv, SpecId, TxEnv, U256},
};
use std::{collections::BTreeMap, num::NonZeroUsize, thread};

// Mock an account from an integer index that is used as the address.
// Useful for mock iterations.
pub fn mock_account(idx: usize) -> (Address, PlainAccount) {
    let address = Address::from(U160::from(idx));
    (
        address,
        // Filling half full accounts to have enough tokens for tests without worrying about
        // the corner case of balance not going beyond `U256::MAX`.
        PlainAccount::from(AccountInfo::from_balance(U256::MAX.div_ceil(U256::from(2)))),
    )
}

pub fn assert_execution_result(sequential_result: &PevmResult, parallel_result: &PevmResult) {
    assert_eq!(sequential_result, parallel_result);
}

// Execute an REVM block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_revm<S: Storage + Clone + Send + Sync>(
    storage: S,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_execution_result(
        &pevm::execute_revm_sequential(storage.clone(), spec_id, block_env.clone(), txs.clone()),
        &pevm::execute_revm(storage, spec_id, block_env, txs, concurrency_level),
    );
}

// Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
// https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
fn calculate_receipt_root(
    txs: &BlockTransactions<Transaction>,
    tx_results: &[PevmTxExecutionResult],
) -> B256 {
    // 1. Create an iterator of ReceiptEnvelope
    let tx_type_iter = txs
        .txns()
        .unwrap()
        .map(|tx| TxType::try_from(tx.transaction_type.unwrap_or_default()).unwrap());

    let receipt_iter = tx_results.iter().map(|tx| tx.receipt.clone().with_bloom());

    let receipt_envelope_iter =
        Iterator::zip(tx_type_iter, receipt_iter).map(|(tx_type, receipt)| match tx_type {
            TxType::Legacy => ReceiptEnvelope::Legacy(receipt),
            TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt),
            TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt),
            TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt),
        });

    // 2. Create a trie then calculate the root hash
    // We use BTreeMap because the keys must be sorted in ascending order.
    let trie_entries: BTreeMap<_, _> = receipt_envelope_iter
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
    hash_builder.root()
}

// Execute an Alloy block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_alloy<S: Storage + Clone + Send + Sync>(
    storage: S,
    block: Block,
    must_match_block_header: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let sequential_result = pevm::execute(storage.clone(), block.clone(), concurrency_level, true);
    let parallel_result = pevm::execute(storage, block.clone(), concurrency_level, false);
    assert_execution_result(&sequential_result, &parallel_result);

    if must_match_block_header {
        let tx_results = sequential_result.unwrap();

        // We can only calculate the receipts root from Byzantium.
        // Before EIP-658 (https://eips.ethereum.org/EIPS/eip-658), the
        // receipt root is calculated with the post transaction state root,
        // which we doesn't have in these tests.
        if block.header.number.unwrap() >= 4370000 {
            assert_eq!(
                block.header.receipts_root,
                calculate_receipt_root(&block.transactions, &tx_results)
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
                .unwrap()
                .receipt
                .cumulative_gas_used
        );
    }
}
