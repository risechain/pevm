use block_stm_revm::storage::Storage;
use block_stm_revm::BlockSTM;
use revm::primitives::{env::TxEnv, ResultAndState};
use revm::{Evm, InMemoryDB};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

// The source-of-truth execution result that BlockSTM must match.
fn execute_sequential(storage: Storage, txs: Arc<Vec<TxEnv>>) -> Vec<ResultAndState> {
    let mut db = InMemoryDB {
        accounts: storage.accounts,
        contracts: storage.contracts,
        logs: Default::default(),
        block_hashes: storage.block_hashes,
        db: Default::default(),
    };

    txs.iter()
        .map(|tx| {
            let result_and_state = Evm::builder()
                .with_db(&mut db)
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
    let (erc20_example_storage, erc20_example_txs) = block_stm_revm::examples::erc20::generate();

    let start_time = SystemTime::now();
    let result_sequential = execute_sequential(
        erc20_example_storage.clone(),
        Arc::new(erc20_example_txs.clone()),
    );
    println!("Executed sequentially in {:?}", start_time.elapsed());

    let start_time = SystemTime::now();
    let result_block_stm = BlockSTM::run(
        erc20_example_storage.clone(),
        Arc::new(erc20_example_txs.clone()),
        thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
    );
    println!("Executed Block-STM in {:?}", start_time.elapsed());

    assert_eq!(
        result_sequential, result_block_stm,
        "Block-STM's execution result doesn't match Sequential's"
    );
}
