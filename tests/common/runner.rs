use alloy_chains::Chain;
use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::{Bloom, Bytes, B256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{Block, BlockTransactions, Transaction};
use pevm::{EvmAccount, PevmResult, PevmTxExecutionResult, Storage};
use revm::primitives::{alloy_primitives::U160, Address, BlockEnv, SpecId, TxEnv, U256};
use std::{collections::BTreeMap, num::NonZeroUsize, thread};

// Mock an account from an integer index that is used as the address.
// Useful for mock iterations.
pub fn mock_account(idx: usize) -> (Address, EvmAccount) {
    let address = Address::from(U160::from(idx));
    let mut account = EvmAccount::default();
    // Filling half full accounts to have enough tokens for tests without worrying about
    // the corner case of balance not going beyond `U256::MAX`.
    account.basic.balance = U256::MAX.div_ceil(U256::from(2));
    (address, account)
}

pub fn assert_execution_result(sequential_result: &PevmResult, parallel_result: &PevmResult) {
    assert_eq!(sequential_result, parallel_result);
}

// Execute an REVM block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_revm<S: Storage + Clone + Send + Sync>(storage: S, txs: Vec<TxEnv>) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_execution_result(
        &pevm::execute_revm_sequential(
            storage.clone(),
            Chain::mainnet(),
            SpecId::LATEST,
            BlockEnv::default(),
            txs.clone(),
        ),
        &pevm::execute_revm(
            storage,
            Chain::mainnet(),
            SpecId::LATEST,
            BlockEnv::default(),
            txs,
            concurrency_level,
        ),
    );
}

fn encode_receipt_2718(tx_type: u8, tx_result: &PevmTxExecutionResult) -> Bytes {
    let tx_type = TxType::try_from(tx_type).unwrap();
    let receipt_with_bloom = tx_result.receipt.clone().with_bloom();
    let receipt_envelope = match tx_type {
        TxType::Legacy => ReceiptEnvelope::Legacy(receipt_with_bloom),
        TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt_with_bloom),
        TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt_with_bloom),
        TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt_with_bloom),
    };

    let mut buffer = Vec::new();
    receipt_envelope.encode_2718(&mut buffer);
    Bytes::from(buffer)
}

// Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
// https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
fn calculate_receipt_root(
    txs: &BlockTransactions<Transaction>,
    tx_results: &[PevmTxExecutionResult],
) -> B256 {
    let trie_entries: BTreeMap<_, _> = txs
        .txns()
        .map(|tx| tx.transaction_type.unwrap_or_default())
        .zip(tx_results)
        .enumerate()
        .map(|(index, (tx_type, tx_result))| {
            let key_buffer = alloy_rlp::encode_fixed_size(&index).to_vec();
            let value_buffer = encode_receipt_2718(tx_type, tx_result);
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
    chain: Chain,
    block: Block,
    must_match_block_header: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let sequential_result = pevm::execute(
        storage.clone(),
        chain,
        block.clone(),
        concurrency_level,
        true,
    );
    let parallel_result = pevm::execute(storage, chain, block.clone(), concurrency_level, false);
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
                .map(|result| result.receipt.cumulative_gas_used)
                .unwrap_or_default()
        );
    }
}
