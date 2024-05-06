use std::{num::NonZeroUsize, thread};

use block_stm_revm::{BlockSTM, Storage};
use revm::{
    primitives::{
        alloy_primitives::U160, Account, AccountInfo, Address, BlockEnv, ResultAndState, SpecId,
        TxEnv, U256,
    },
    DatabaseCommit, Evm, InMemoryDB,
};

// Mock an account from an integer index that is used as the address.
// Useful for mock iterations.
pub(crate) fn mock_account(idx: usize) -> (Address, Account) {
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
fn execute_sequential(
    mut db: InMemoryDB,
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: &[TxEnv],
) -> Vec<ResultAndState> {
    txs.iter()
        .map(|tx| {
            let result_and_state = Evm::builder()
                .with_ref_db(&mut db)
                .with_spec_id(spec_id)
                .with_block_env(block_env.clone())
                .with_tx_env(tx.clone())
                .build()
                .transact()
                // TODO: Proper error handling
                .unwrap();
            db.commit(result_and_state.state.clone());
            result_and_state
        })
        .collect()
}

// Execute a list of transactions sequentially & with BlockSTM and assert that
// the execution results match.
pub(crate) fn test_txs(
    accounts: &[(Address, Account)],
    spec_id: SpecId,
    block_env: BlockEnv,
    txs: Vec<TxEnv>,
) {
    // TODO: Decouple the (number of) prefilled accounts with the number of transactions.
    let (sequential_db, block_stm_storage) = setup_storage(accounts);
    let result_sequential = execute_sequential(sequential_db, spec_id, block_env.clone(), &txs);
    let result_block_stm = BlockSTM::run(
        block_stm_storage,
        spec_id,
        block_env,
        txs,
        thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
    );

    assert_eq!(
        result_sequential, result_block_stm,
        "Block-STM's execution result doesn't match Sequential's"
    );
}
