//! A quick & dirty executable to test run BlockSTM.
//! TODO: Turn this into proper test & bench executables.

use block_stm_revm::{BlockSTM, Storage};
use revm::primitives::alloy_primitives::U160;
use revm::primitives::{env::TxEnv, ResultAndState};
use revm::primitives::{Address, BlockEnv};
use revm::Evm;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

// The source-of-truth execution result that BlockSTM must match.
fn execute_sequential(txs: Arc<Vec<TxEnv>>) -> Vec<ResultAndState> {
    let db = Storage::default();
    txs.iter()
        .map(|tx| {
            let result_and_state = Evm::builder()
                .with_ref_db(&db)
                .with_tx_env(tx.clone())
                .build()
                .transact()
                // TODO: Proper error handling
                .unwrap();
            result_and_state
        })
        .collect()
}

fn main() {
    // TODO: Populate real-er transactions please!
    let block_size = 500_000; // number of transactions
    let txs: Arc<Vec<TxEnv>> = Arc::new(
        (0..block_size)
            .map(|idx| TxEnv {
                // Avoid Address::ZERO
                caller: Address::from(U160::from(idx + 1)),
                ..TxEnv::default()
            })
            .collect(),
    );

    let start_time = SystemTime::now();
    let result_sequential = execute_sequential(txs.clone());
    println!("Executed sequentially in {:?}", start_time.elapsed());

    let start_time = SystemTime::now();
    let result_block_stm = BlockSTM::run(
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
