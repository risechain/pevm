//! Run tests against GeneralStateTests/

mod coercion;

use block_stm_revm::{BlockSTM, Storage};
use clap::Parser;
use coercion::from_storage;
use revm::primitives::{
    calc_excess_blob_gas, AccountInfo, Address, BlobExcessGasAndPrice, BlockEnv, Bytecode,
    ResultAndState, TransactTo, TxEnv, U256,
};
use revm::DatabaseCommit;
use revme::cmd::statetest::merkle_trie::{log_rlp_hash, state_merkle_trie_root};
use revme::cmd::statetest::{models as smodels, utils::recover_address};
use std::{collections::HashMap, fs, num::NonZeroUsize, path::Path, process::ExitCode};
use thiserror::Error;

use crate::coercion::{to_plain_account, to_storage};

#[derive(Parser)]
struct Args {
    paths: Vec<String>,
}

#[derive(Debug, Error)]
pub(crate) enum StateTestError {
    #[error(transparent)]
    StdIo(#[from] std::io::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

fn build_storage(pre: &HashMap<Address, smodels::AccountInfo>) -> Storage {
    let mut storage = Storage::default();

    for (k, v) in pre.iter() {
        let code = Bytecode::new_raw(v.code.clone());
        let info = AccountInfo::new(v.balance, v.nonce, code.hash_slow(), code.clone());
        storage.insert_account_info(*k, info);
    }

    storage
}

fn build_block_env(env: &smodels::Env) -> BlockEnv {
    let resolved_blob_excess_gas_and_price = {
        if let Some(t) = env.current_excess_blob_gas {
            Some(BlobExcessGasAndPrice::new(t.to()))
        } else if let (Some(x), Some(y)) = (env.parent_blob_gas_used, env.parent_excess_blob_gas) {
            Some(BlobExcessGasAndPrice::new(calc_excess_blob_gas(
                x.to(),
                y.to(),
            )))
        } else {
            None
        }
    };

    BlockEnv {
        number: env.current_number,
        coinbase: env.current_coinbase,
        timestamp: env.current_timestamp,
        gas_limit: env.current_gas_limit,
        basefee: env.current_base_fee.unwrap_or_default(),
        difficulty: env.current_difficulty,
        prevrandao: env.current_random,
        blob_excess_gas_and_price: resolved_blob_excess_gas_and_price,
    }
}

fn build_tx_env(tx: &smodels::TransactionParts, indices: &smodels::TxPartIndices) -> TxEnv {
    TxEnv {
        caller: if let Some(sender) = tx.sender {
            sender
        } else if let Some(addr) = recover_address(tx.secret_key.as_slice()) {
            addr
        } else {
            panic!("unknown private key")
        },
        gas_limit: tx.gas_limit[indices.gas].saturating_to(),
        gas_price: if let Some(gas_price) = tx.gas_price {
            gas_price
        } else if let Some(max_fee_per_gas) = tx.max_fee_per_gas {
            max_fee_per_gas
        } else {
            U256::default()
        },
        transact_to: match tx.to {
            Some(add) => TransactTo::Call(add),
            None => TransactTo::Create,
        },
        value: tx.value[indices.value],
        data: tx.data[indices.data].clone(),
        nonce: None,
        chain_id: None,
        access_list: Vec::default(),
        gas_priority_fee: tx.max_priority_fee_per_gas,
        blob_hashes: tx.blob_versioned_hashes.clone(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
        eof_initcodes: Vec::default(),
        eof_initcodes_hashed: HashMap::default(),
    }
}

fn run_test_unit(unit: smodels::TestUnit) -> Result<(), StateTestError> {
    for (spec_name, tests) in unit.post {
        if matches!(
            spec_name,
            smodels::SpecName::ByzantiumToConstantinopleAt5
                | smodels::SpecName::Constantinople
                | smodels::SpecName::Unknown
        ) {
            continue;
        }

        for test in tests {
            let spec_id = spec_name.to_spec_id();
            let mut db = from_storage(build_storage(&unit.pre));
            let block_env = build_block_env(&unit.env);
            let tx_env = build_tx_env(&unit.transaction, &test.indexes);

            let result_block_stm = BlockSTM::run(
                to_storage(db.clone()),
                spec_id,
                block_env,
                Vec::from([tx_env]),
                NonZeroUsize::MIN,
            );

            assert!(result_block_stm.len() == 1);
            let ResultAndState { result, state } = result_block_stm[0].clone();
            db.commit(state);

            let logs_root = log_rlp_hash(result.logs());
            assert_eq!(logs_root, test.logs);

            let plain_accounts: Vec<_> = db
                .accounts
                .iter()
                .map(|(k, v)| (*k, to_plain_account(v)))
                .collect();
            let state_root = state_merkle_trie_root(plain_accounts.iter().map(|(k, v)| (*k, v)));
            assert_eq!(state_root, test.hash);
        }
    }

    Ok(())
}

fn run_test_suite(suite: smodels::TestSuite) -> Result<(), StateTestError> {
    for (k, v) in suite.0 {
        println!("Running: {}", k);
        run_test_unit(v)?;
    }
    Ok(())
}

fn run_test(path: &Path) -> Result<(), StateTestError> {
    let text = fs::read_to_string(path)?;
    let suite: smodels::TestSuite = serde_json::from_str(&text)?;
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
