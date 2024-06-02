use alloy_rpc_types::{Block, Header};
use pevm::{get_block_env, get_block_spec, get_tx_envs, InMemoryStorage, PevmError, PevmResult};
use revm::{
    db::{PlainAccount, WrapDatabaseRef},
    primitives::{
        alloy_primitives::U160, AccountInfo, Address, BlockEnv, CfgEnv, EVMError, Env,
        ResultAndState, SpecId, TxEnv, U256,
    },
    Context, DatabaseCommit, DatabaseRef, Evm, EvmContext, Handler, InMemoryDB,
};
use std::{fmt::Debug, num::NonZeroUsize, thread};

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

// Build an in-memory database for sequential execution and an in-memory
// storage for parallel execution.
pub fn build_in_mem(state: ChainState) -> (InMemoryDB, InMemoryStorage) {
    let mut db = InMemoryDB::default();
    for (address, account) in state.iter() {
        db.insert_account_info(*address, account.info.clone());
        for (slot, value) in account.storage.iter() {
            // TODO: Better error handling
            db.insert_account_storage(*address, *slot, *value).unwrap();
        }
    }
    (db, state.into())
}

// The source-of-truth sequential execution result that PEVM must match.
// Currently early returning the first encountered EVM error if there is any.
pub fn execute_sequential<D: DatabaseRef + DatabaseCommit>(
    mut db: D,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) -> Result<Vec<ResultAndState>, EVMError<D::Error>> {
    let mut results = Vec::new();
    for tx in txs {
        // This is much uglier than the builder interface but can be up to 50% faster!!
        let context = Context {
            evm: EvmContext::new_with_env(
                WrapDatabaseRef(&db),
                Env::boxed(
                    // TODO: Should we turn off byte code analysis?
                    CfgEnv::default(),
                    block_env.clone(),
                    tx,
                ),
            ),
            external: (),
        };
        let handler = Handler::mainnet_with_spec(spec_id, true);
        let result = Evm::new(context, handler).transact();
        match result {
            Ok(result_and_state) => {
                db.commit(result_and_state.state.clone());
                results.push(result_and_state);
            }
            Err(err) => return Err(err),
        }
    }
    Ok(results)
}

// TODO: More elegant solution?
fn assert_evm_errors<DBError1: Debug, DBError2: Debug>(
    seq_err: &EVMError<DBError1>,
    pevm_err: &EVMError<DBError2>,
) {
    match (seq_err, pevm_err) {
        (EVMError::Transaction(e1), EVMError::Transaction(e2)) => assert_eq!(e1, e2),
        (EVMError::Header(e1), EVMError::Header(e2)) => assert_eq!(e1, e2),
        (EVMError::Custom(e1), EVMError::Custom(e2)) => assert_eq!(e1, e2),
        // We treat all database errors as inequality.
        // Warning: This can be dangerous when `EVMError` introduces a new variation.
        (e1, e2) => panic!(
            "PEVM's execution error doesn't match Sequential's\nSequential: {e1:?}\nPEVM: {e2:?}"
        ),
    }
}

pub fn assert_execution_result<D: DatabaseRef>(
    sequential_result: Result<Vec<ResultAndState>, EVMError<D::Error>>,
    pevm_result: PevmResult,
    must_succeed: bool,
) where
    D::Error: Debug,
{
    match (sequential_result, pevm_result) {
        (Ok(sequential_results), Ok(parallel_results)) => {
            assert_eq!(sequential_results, parallel_results)
        }
        // TODO: Support extracting and comparing multiple errors in the input block.
        // This only works for now as most tests have just one (potentially error) transaction.
        (Err(sequential_error), Err(PevmError::ExecutionError(parallel_error))) => {
            if must_succeed {
                panic!("This block must succeed!");
            } else {
                assert_evm_errors(&sequential_error, &parallel_error);
            }
        }
        _ => panic!("PEVM's execution result doesn't match Sequential's"),
    };
}

// Execute an REVM block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_revm(state: ChainState, spec_id: SpecId, block_env: BlockEnv, txs: Vec<TxEnv>) {
    let (db, storage) = build_in_mem(state);
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_execution_result::<InMemoryDB>(
        execute_sequential(db, spec_id, block_env.clone(), txs.clone()),
        pevm::execute_revm(storage, spec_id, block_env, txs, concurrency_level),
        false, // TODO: Parameterize this
    );
}

// Execute an Alloy block sequentially & with PEVM and assert that
// the execution results match.
pub fn test_execute_alloy(
    state: ChainState,
    block: Block,
    parent_header: Option<Header>,
    must_succeed: bool,
) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let (db, storage) = build_in_mem(state);
    assert_execution_result::<InMemoryDB>(
        execute_sequential(
            db.clone(),
            get_block_spec(&block.header).unwrap(),
            get_block_env(&block.header, parent_header.as_ref()).unwrap(),
            get_tx_envs(&block.transactions).unwrap(),
        ),
        pevm::execute(storage, block, parent_header, concurrency_level),
        must_succeed,
    );
}
