use alloy_rpc_types::Block;
use pevm::{PevmResult, Storage};
use revm::{
    db::PlainAccount,
    primitives::{alloy_primitives::U160, AccountInfo, Address, BlockEnv, SpecId, TxEnv, U256},
};
use std::{num::NonZeroUsize, thread};

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
    sequential_result: PevmResult,
    parallel_result: PevmResult,
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
        pevm::execute_revm_sequential(storage.clone(), spec_id, block_env.clone(), txs.clone()),
        pevm::execute_revm(storage, spec_id, block_env, txs, concurrency_level),
        false, // TODO: Parameterize this
    );
}

// Execute an Alloy block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_alloy<S: Storage + Clone + Send + Sync>(
    storage: S,
    block: Block,
    must_succeed: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_execution_result(
        pevm::execute(storage.clone(), block.clone(), concurrency_level, true),
        pevm::execute(storage, block, concurrency_level, false),
        must_succeed,
    );
}
