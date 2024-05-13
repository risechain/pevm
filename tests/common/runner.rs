use alloy_rpc_types::{Block, Header};
use block_stm_revm::{
    get_block_env, get_block_spec, get_tx_envs, BlockStmError, BlockStmResult, Storage,
};
use revm::{
    primitives::{
        alloy_primitives::U160, Account, AccountInfo, Address, BlockEnv, EVMError, ResultAndState,
        SpecId, TxEnv, U256,
    },
    DatabaseCommit, Evm, InMemoryDB,
};
use std::{convert::Infallible, num::NonZeroUsize, thread};

type SequentialExecutionResult = Result<Vec<ResultAndState>, EVMError<Infallible>>;

// Mock an account from an integer index that is used as the address.
// Useful for mock iterations.
pub fn mock_account(idx: usize) -> (Address, Account) {
    let address = Address::from(U160::from(idx));
    (
        address,
        // Filling half full accounts to have enough tokens for tests without worrying about
        // the corner case of balance not going beyond `U256::MAX`.
        Account::from(AccountInfo::from_balance(U256::MAX.div_ceil(U256::from(2)))),
    )
}

// Return an `InMemoryDB` for sequential usage and `Storage` for BlockSTM usage.
// Both represent a "standard" mock state with prefilled accounts.
fn setup_storage(accounts: &[(Address, Account)]) -> (InMemoryDB, Storage) {
    let mut sequential_db: revm::db::CacheDB<revm::db::EmptyDBTyped<std::convert::Infallible>> =
        InMemoryDB::default();
    let mut block_stm_storage = Storage::default();

    for (address, account) in accounts {
        sequential_db.insert_account_info(*address, account.info.clone());
        for (slot, value) in account.storage.iter() {
            // TODO: Better error handling
            sequential_db
                .insert_account_storage(*address, *slot, value.present_value)
                .unwrap();
        }
        block_stm_storage.insert_account(*address, account.clone());
    }

    (sequential_db, block_stm_storage)
}

// The source-of-truth sequential execution result that BlockSTM must match.
// Currently early returning the first encountered EVM error if there is any.
fn execute_sequential(
    mut db: InMemoryDB,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: &[TxEnv],
) -> SequentialExecutionResult {
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
fn eq_evm_errors<DBError1, DBError2>(e1: &EVMError<DBError1>, e2: &EVMError<DBError2>) -> bool {
    match (e1, e2) {
        (EVMError::Transaction(e1), EVMError::Transaction(e2)) => e1 == e2,
        (EVMError::Header(e1), EVMError::Header(e2)) => e1 == e2,
        (EVMError::Custom(e1), EVMError::Custom(e2)) => e1 == e2,
        // We treat all database errors as inequality.
        // Warning: This can be dangerous when `EVMError` introduces a new variation.
        _ => false,
    }
}

fn assert_execution_result(
    sequential_result: SequentialExecutionResult,
    block_stm_result: BlockStmResult,
) {
    match (sequential_result, block_stm_result) {
        (Ok(sequential_results), Ok(parallel_results)) => {
            assert_eq!(sequential_results, parallel_results)
        }
        // TODO: Support extracting and comparing multiple errors in the input block.
        // This only works for now as most tests have just one (potentially error) transaction.
        (Err(sequential_error), Err(BlockStmError::ExecutionError(parallel_error))) => {
            assert!(eq_evm_errors(&sequential_error, &parallel_error))
        }
        _ => panic!("Block-STM's execution result doesn't match Sequential's"),
    };
}

// Execute an REVM block sequentially & with BlockSTM and assert that
// the execution results match.
pub fn test_execute_revm(
    accounts: &[(Address, Account)],
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) {
    let (sequential_db, block_stm_storage) = setup_storage(accounts);
    assert_execution_result(
        execute_sequential(sequential_db, spec_id, block_env.clone(), &txs),
        block_stm_revm::execute_revm(
            block_stm_storage,
            spec_id,
            block_env,
            txs,
            thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
        ),
    );
}

// Execute an Alloy block sequentially & with BlockSTM and assert that
// the execution results match.
pub fn test_execute_alloy(
    accounts: &[(Address, Account)],
    block: Block,
    parent_header: Option<Header>,
) {
    let (sequential_db, block_stm_storage) = setup_storage(accounts);
    assert_execution_result(
        execute_sequential(
            sequential_db,
            get_block_spec(&block.header).unwrap(),
            get_block_env(&block.header, parent_header.as_ref()).unwrap(),
            &get_tx_envs(&block.transactions).unwrap(),
        ),
        block_stm_revm::execute(
            block_stm_storage,
            block,
            parent_header,
            thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
        ),
    );
}
