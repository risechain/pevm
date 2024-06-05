use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::B256;
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{Block, Header};
use pevm::{InMemoryStorage, PevmResult, PevmTxExecutionResult, Storage};
use revm::{
    db::PlainAccount,
    primitives::{alloy_primitives::U160, AccountInfo, Address, BlockEnv, SpecId, TxEnv, U256},
};
use std::{collections::BTreeMap, num::NonZeroUsize, thread};

use super::ChainState;

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

// TODO: Pass in hashes to checksum, especially for real blocks.
pub fn assert_execution_result(
    sequential_result: &PevmResult,
    parallel_result: &PevmResult,
    must_succeed: bool,
) {
    // We must assert sucess for real blocks, etc.
    if must_succeed {
        assert!(sequential_result.is_ok() && parallel_result.is_ok());
    }
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
        false, // TODO: Parameterize this
    );
}

fn calculate_receipt_root(receipt_envelopes: Vec<ReceiptEnvelope>) -> B256 {
    // create a BTreeMap (order must be ascending)
    let entries = receipt_envelopes
        .into_iter()
        .enumerate()
        .map(|(index, receipt)| {
            let key_buffer = alloy_rlp::encode_fixed_size(&index);
            let mut value_buffer = Vec::new();
            receipt.encode_2718(&mut value_buffer);
            (key_buffer, value_buffer)
        })
        .collect::<BTreeMap<_, _>>();

    // calculate the MPT root
    let mut hash_builder = alloy_trie::HashBuilder::default();
    for (k, v) in entries {
        hash_builder.add_leaf(alloy_trie::Nibbles::unpack(&k), &v);
    }
    hash_builder.root()
}

fn get_receipt_envelopes(
    block: &Block,
    tx_results: &[PevmTxExecutionResult],
) -> Vec<ReceiptEnvelope> {
    // get the receipts
    let receipt_iter = tx_results.iter().map(|tx| tx.receipt.clone());

    // get the tx_type list
    let tx_type_iter = block
        .transactions
        .txns()
        .unwrap()
        .map(|tx| TxType::try_from(tx.transaction_type.unwrap_or_default()).unwrap());

    // zip them to get the receipt envelopes
    let receipt_envelope_iter =
        Iterator::zip(receipt_iter, tx_type_iter).map(|(receipt, tx_type)| match tx_type {
            TxType::Legacy => ReceiptEnvelope::Legacy(receipt.with_bloom()),
            TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt.with_bloom()),
            TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt.with_bloom()),
            TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt.with_bloom()),
        });
    receipt_envelope_iter.collect::<Vec<_>>()
}

// Execute an Alloy block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_alloy<S: Storage + Clone + Send + Sync>(
    storage: S,
    block: Block,
    parent_header: Option<Header>,
    must_succeed: bool,
    must_check_receipts_root: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let sequential_result = pevm::execute(
        storage.clone(),
        block.clone(),
        parent_header.clone(),
        concurrency_level,
        true,
    );
    let parallel_result = pevm::execute(
        storage.clone(),
        block.clone(),
        parent_header.clone(),
        concurrency_level,
        false,
    );
    assert_execution_result(&sequential_result, &parallel_result, must_succeed);

    if must_check_receipts_root {
        if block.header.number.unwrap() < 4370000 { // before Byzantium
             // Before EIP 658 (https://eips.ethereum.org/EIPS/eip-658),
             // the receipt root is calculated from post transaction state root.
             // Unfortunately, this info is not available in type Receipt.

            // Note that in this era: the receipt root equals to:
            // TRIE { (k, v) for all k=0..N-1, v=(post transaction state root, cumulative gas used, bloom filter, logs) }

            // For those who are curious, call `eth_getTransactionReceipt`
            // and find the field `root`, that is the missing piece which
            // is needed to calculate the `receiptsRoot`.
        } else {
            let receipt_envelopes = get_receipt_envelopes(&block, &sequential_result.unwrap());
            let calculated_receipts_root = calculate_receipt_root(receipt_envelopes);
            assert_eq!(block.header.receipts_root, calculated_receipts_root);
        }
    }
}
