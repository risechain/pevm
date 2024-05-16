use alloy_rpc_types::{Block, Header};
use block_stm_revm::{get_block_env, get_block_spec, get_tx_envs, BlockStmError, BlockStmResult};
use revm::{
    db::PlainAccount,
    primitives::{
        alloy_primitives::U160, AccountInfo, Address, BlockEnv, EVMError, ResultAndState, SpecId,
        TxEnv, U256,
    },
    DatabaseCommit, DatabaseRef, Evm, InMemoryDB,
};
use std::{fmt::Debug, num::NonZeroUsize, thread};

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

// Build an inmemory database from a preset state of accounts.
pub fn build_inmem_db(accounts: impl IntoIterator<Item = (Address, PlainAccount)>) -> InMemoryDB {
    let mut db = InMemoryDB::default();
    for (address, account) in accounts {
        db.insert_account_info(address, account.info.clone());
        for (slot, value) in account.storage.iter() {
            // TODO: Better error handling
            db.insert_account_storage(address, *slot, *value).unwrap();
        }
    }
    db
}

// The source-of-truth sequential execution result that BlockSTM must match.
// Currently early returning the first encountered EVM error if there is any.
pub fn execute_sequential<D: DatabaseRef + DatabaseCommit>(
    mut db: D,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: &[TxEnv],
) -> Result<Vec<ResultAndState>, EVMError<D::Error>> {
    let mut results = Vec::new();
    for tx in txs {
        let mut evm = Evm::builder()
            .with_ref_db(&mut db)
            .with_spec_id(spec_id)
            .with_block_env(block_env.clone())
            .with_tx_env(tx.clone())
            .build();
        let result = evm.transact();
        drop(evm); // to reclaim the DB

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
    block_stm_err: &EVMError<DBError2>,
) {
    match (seq_err, block_stm_err) {
        (EVMError::Transaction(e1), EVMError::Transaction(e2)) => assert_eq!(e1, e2),
        (EVMError::Header(e1), EVMError::Header(e2)) => assert_eq!(e1, e2),
        (EVMError::Custom(e1), EVMError::Custom(e2)) => assert_eq!(e1, e2),
        // We treat all database errors as inequality.
        // Warning: This can be dangerous when `EVMError` introduces a new variation.
        (e1, e2) =>
            panic!("Block-STM's execution error doesn't match Sequential's\nSequential: {e1:?}\nBlock-STM: {e2:?}"),
    }
}

fn assert_execution_result<D: DatabaseRef>(
    sequential_result: Result<Vec<ResultAndState>, EVMError<D::Error>>,
    block_stm_result: BlockStmResult,
) where
    D::Error: Debug,
{
    match (sequential_result, block_stm_result) {
        (Ok(sequential_results), Ok(parallel_results)) => {
            assert_eq!(sequential_results, parallel_results)
        }
        // TODO: Support extracting and comparing multiple errors in the input block.
        // This only works for now as most tests have just one (potentially error) transaction.
        (Err(sequential_error), Err(BlockStmError::ExecutionError(parallel_error))) => {
            assert_evm_errors(&sequential_error, &parallel_error);
        }
        _ => panic!("Block-STM's execution result doesn't match Sequential's"),
    };
}

// Execute an REVM block sequentially & with BlockSTM and assert that
// the execution results match.
pub fn test_execute_revm<D: DatabaseRef + DatabaseCommit + Send + Sync + Clone>(
    db: D,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) where
    D::Error: Debug,
{
    assert_execution_result::<D>(
        execute_sequential(db.clone(), spec_id, block_env.clone(), &txs),
        block_stm_revm::execute_revm(
            db,
            spec_id,
            block_env,
            txs,
            thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
        ),
    );
}

// Execute an Alloy block sequentially & with BlockSTM and assert that
// the execution results match.
pub fn test_execute_alloy<D: DatabaseRef + DatabaseCommit + Send + Sync + Clone>(
    db: D,
    block: Block,
    parent_header: Option<Header>,
) where
    D::Error: Debug,
{
    assert_execution_result::<D>(
        execute_sequential(
            db.clone(),
            get_block_spec(&block.header).unwrap(),
            get_block_env(&block.header, parent_header.as_ref()).unwrap(),
            &get_tx_envs(&block.transactions).unwrap(),
        ),
        block_stm_revm::execute(
            db,
            block,
            parent_header,
            thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
        ),
    );
}
