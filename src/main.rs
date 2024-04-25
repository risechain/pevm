//! A quick & dirty executable to test run BlockSTM.
//! TODO: Turn this into proper test & bench executables.

use block_stm_revm::{BlockSTM, Storage};
use revm::primitives::{
    alloy_primitives::U160, env::TxEnv, AccountInfo, Address, BlockEnv, ResultAndState, TransactTo,
    U256,
};
use revm::{Evm, InMemoryDB};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

// The source-of-truth execution result that BlockSTM must match.
fn execute_sequential(mut db: InMemoryDB, txs: Arc<Vec<TxEnv>>) -> Vec<ResultAndState> {
    txs.iter()
        .map(|tx| {
            let result_and_state = Evm::builder()
                .with_ref_db(&mut db)
                .with_tx_env(tx.clone())
                .build()
                .transact()
                // TODO: Proper error handling
                .unwrap();
            // TODO: Commit db
            result_and_state
        })
        .collect()
}

fn main() {
    // TODO: Populate real-er transactions please!
    let block_size = 500_000; // number of transactions

    // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
    let mut sequential_db = InMemoryDB::default();
    let mut block_stm_storage = Storage::default();
    // Half full to have enough tokens for tests without the corner case of not going beyond MAX.
    let mock_account = AccountInfo::from_balance(U256::MAX.div_ceil(U256::from(2)));
    for i in 0..=block_size {
        let address = Address::from(U160::from(i));
        sequential_db.insert_account_info(address, mock_account.clone());
        block_stm_storage.insert_account_info(address, mock_account.clone());
    }

    // Mock `block_size` transactions sending some tokens to itself.
    // Avoiding `Address:ZERO` to act as the beneficiary account.
    let txs: Arc<Vec<TxEnv>> = Arc::new(
        (1..=block_size)
            .map(|i| {
                let address = Address::from(U160::from(i));
                TxEnv {
                    caller: address,
                    transact_to: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_price: U256::from(1),
                    ..TxEnv::default()
                }
            })
            .collect(),
    );

    let start_time = SystemTime::now();
    let result_sequential = execute_sequential(sequential_db, txs.clone());
    println!("Executed sequentially in {:?}", start_time.elapsed());

    let start_time = SystemTime::now();
    let result_block_stm = BlockSTM::run(
        Arc::new(block_stm_storage),
        BlockEnv::default(),
        txs,
        thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
    );
    println!("Executed Block-STM in {:?}", start_time.elapsed());

    assert_eq!(
        result_sequential, result_block_stm,
        "Block-STM's execution result doesn't match Sequential's"
    );
}
