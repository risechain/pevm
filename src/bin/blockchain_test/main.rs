//! Run tests against BlockchainTests/

mod coercion;
mod models;

use crate::coercion::to_storage;
use crate::models::{BlockchainTestError, BlockchainTestSuite};
use block_stm_revm::{BlockSTM, Storage};
use clap::Parser;
use coercion::from_storage;
use models::BlockchainTestUnit;
use revm::primitives::{
    AccountInfo, Address, BlobExcessGasAndPrice, BlockEnv, Bytecode, TxEnv, U256,
};
use revm::{Database, DatabaseCommit};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::ExitCode;
use std::{fs, thread};

#[derive(Parser)]
struct Args {
    paths: Vec<String>,
}

fn build_storage(pre: &HashMap<Address, models::AccountInfo>) -> Storage {
    let mut storage = Storage::default();

    for (k, v) in pre.iter() {
        let code = Bytecode::new_raw(v.code.clone());
        let info = AccountInfo::new(v.balance, v.nonce, code.hash_slow(), code.clone());
        storage.insert_account_info(*k, info);
    }

    storage
}

fn get_tx_env(tx: models::Transaction) -> TxEnv {
    TxEnv {
        caller: tx.sender,
        gas_limit: tx.gas_limit.unwrap_or_default().to(),
        gas_price: tx.gas_price.unwrap_or_default(),
        transact_to: revm::primitives::TransactTo::Call(tx.to),
        value: tx.value,
        data: tx.data,
        nonce: Some(tx.nonce.to()),
        chain_id: tx.chain_id.map(|x| x.to()),
        access_list: tx
            .access_list
            .unwrap_or_default()
            .iter()
            .map(|item| {
                (
                    item.address,
                    item.storage_keys
                        .iter()
                        .map(|key| U256::from_be_bytes(key.0))
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
        gas_priority_fee: tx.max_priority_fee_per_gas,
        blob_hashes: tx.blob_versioned_hashes.unwrap_or_default(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
        eof_initcodes: Vec::default(),
        eof_initcodes_hashed: HashMap::default(),
    }
}

fn run_test_unit(unit: BlockchainTestUnit) -> Result<(), BlockchainTestError> {
    let spec_id = unit.network.to_spec_id();
    let concurrency = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let mut db = from_storage(build_storage(&unit.pre));

    for block in unit.blocks {
        let block_env = BlockEnv {
            number: block.block_header.number,
            coinbase: block.block_header.coinbase,
            timestamp: block.block_header.timestamp,
            gas_limit: block.block_header.gas_limit,
            basefee: block.block_header.base_fee_per_gas.unwrap_or_default(),
            difficulty: block.block_header.difficulty,
            prevrandao: block.block_header.mix_hash,
            blob_excess_gas_and_price: block
                .block_header
                .excess_blob_gas
                .map(|x| BlobExcessGasAndPrice::new(x.to())),
        };

        let txs: Vec<TxEnv> = block.transactions.into_iter().map(get_tx_env).collect();

        let result_block_stm =
            BlockSTM::run(to_storage(db.clone()), spec_id, block_env, txs, concurrency);

        println!("{:?}", result_block_stm);

        for result in result_block_stm {
            println!("result.state = {:?}", result.state);
            db.commit(result.state);
        }
    }

    println!("{:?}", db.accounts);

    for (address, expected_info) in unit.post_state {
        let observed_info = db.basic(address, false).unwrap().unwrap();
        assert_eq!(
            observed_info.balance, expected_info.balance,
            "address={}",
            address
        );
    }

    Ok(())
}

fn run_test_suite(suite: BlockchainTestSuite) -> Result<(), BlockchainTestError> {
    for (k, v) in suite.0 {
        println!("Running: {}", k);
        if v.network.to_spec_id() == revm::primitives::SpecId::BERLIN {
            continue;
        }
        run_test_unit(v)?;
    }
    Ok(())
}

fn run_test(path: &Path) -> Result<(), BlockchainTestError> {
    let text = fs::read_to_string(path)?;
    let suite: BlockchainTestSuite = serde_json::from_str(&text)?;
    run_test_suite(suite)
}

fn main() -> ExitCode {
    let args = Args::parse();

    if args.paths.is_empty() {
        println!("No tests provided. Did you forget to pass the arguments?");
        return ExitCode::SUCCESS;
    }

    for path in args.paths {
        let result = run_test(Path::new(&path));
        if let Err(error) = result {
            eprintln!("{:?}", error);
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
