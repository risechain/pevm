// Basing this off REVM's bins/revme/src/cmd/statetest/runner.rs

use block_stm_revm::{BlockSTM, Storage};
use revm::db::PlainAccount;
use revm::primitives::{
    calc_excess_blob_gas, Account, AccountInfo, Address, BlobExcessGasAndPrice, BlockEnv, Bytecode,
    ResultAndState, TransactTo, TxEnv, U256,
};
use revme::cmd::statetest::{
    merkle_trie::{log_rlp_hash, state_merkle_trie_root},
    models as smodels,
    utils::recover_address,
};
use std::{collections::HashMap, fs, num::NonZeroUsize, path::Path};

fn build_block_env(env: &smodels::Env) -> BlockEnv {
    BlockEnv {
        number: env.current_number,
        coinbase: env.current_coinbase,
        timestamp: env.current_timestamp,
        gas_limit: env.current_gas_limit,
        basefee: env.current_base_fee.unwrap_or_default(),
        difficulty: env.current_difficulty,
        prevrandao: env.current_random,
        blob_excess_gas_and_price: if let Some(current_excess_blob_gas) =
            env.current_excess_blob_gas
        {
            Some(BlobExcessGasAndPrice::new(current_excess_blob_gas.to()))
        } else if let (Some(parent_blob_gas_used), Some(parent_excess_blob_gas)) =
            (env.parent_blob_gas_used, env.parent_excess_blob_gas)
        {
            Some(BlobExcessGasAndPrice::new(calc_excess_blob_gas(
                parent_blob_gas_used.to(),
                parent_excess_blob_gas.to(),
            )))
        } else {
            None
        },
    }
}

fn build_tx_env(tx: &smodels::TransactionParts, indexes: &smodels::TxPartIndices) -> TxEnv {
    TxEnv {
        caller: if let Some(address) = tx.sender {
            address
        } else if let Some(address) = recover_address(tx.secret_key.as_slice()) {
            address
        } else {
            panic!("Failed to parse caller") // TODO: Report test name
        },
        gas_limit: tx.gas_limit[indexes.gas].saturating_to(),
        gas_price: tx.gas_price.or(tx.max_fee_per_gas).unwrap_or_default(),
        transact_to: match tx.to {
            Some(address) => TransactTo::Call(address),
            None => TransactTo::Create,
        },
        value: tx.value[indexes.value],
        data: tx.data[indexes.data].clone(),
        nonce: Some(tx.nonce.saturating_to()),
        chain_id: Some(1), // Ethereum mainnet
        access_list: tx
            .access_lists
            .get(indexes.data)
            .and_then(Option::as_deref)
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
        blob_hashes: tx.blob_versioned_hashes.clone(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
        eof_initcodes: Vec::new(),
        eof_initcodes_hashed: HashMap::new(),
    }
}

fn run_test_unit(unit: smodels::TestUnit) {
    for (spec_name, tests) in unit.post {
        // Should REVM know and handle these better, or it is
        // truly fine to just skip them?
        if matches!(spec_name, smodels::SpecName::Unknown) {
            continue;
        }
        let spec_id = spec_name.to_spec_id();

        for test in tests {
            let mut chain_state: HashMap<Address, PlainAccount> = HashMap::new();
            let mut block_stm_storage = Storage::default();

            // Shouldn't we parse accounts as `Account` instead of `AccountInfo`
            // to have initial storage states?
            for (address, raw_info) in unit.pre.iter() {
                let code = Bytecode::new_raw(raw_info.code.clone());
                let info =
                    AccountInfo::new(raw_info.balance, raw_info.nonce, code.hash_slow(), code);
                chain_state.insert(*address, info.clone().into());
                block_stm_storage.insert_account(*address, Account::from(info));
            }

            let exec_results = BlockSTM::run(
                block_stm_storage,
                spec_id,
                build_block_env(&unit.env),
                vec![build_tx_env(&unit.transaction, &test.indexes)],
                NonZeroUsize::MIN,
            );

            // TODO: We really should test with blocks with more than 1 tx
            assert!(exec_results.len() == 1);
            let ResultAndState { result, state } = exec_results[0].clone();

            let logs_root = log_rlp_hash(result.logs());
            assert_eq!(logs_root, test.logs);

            for (address, account) in state {
                chain_state.insert(
                    address,
                    PlainAccount {
                        info: account.info,
                        storage: account
                            .storage
                            .iter()
                            .map(|(k, v)| (*k, v.present_value))
                            .collect(),
                    },
                );
            }
            let state_root = state_merkle_trie_root(chain_state.iter().map(|(k, v)| (*k, v)));
            assert_eq!(state_root, test.hash);
        }
    }
}

#[test]
fn ethereum_tests() {
    // TODO: Run the whole suite.
    // Skip tests like REVM does when it makes sense.
    // Let's document clearly why for each test that we skip.
    let path_prefix = String::from("tests/ethereum/tests/GeneralStateTests/");
    let state_tests = ["stExample/add11.json"];
    for test in state_tests {
        let path = path_prefix.clone() + test;
        let raw_content = fs::read_to_string(Path::new(&path))
            .unwrap_or_else(|_| panic!("Cannot read suite: {:?}", test));
        let parsed_suite: smodels::TestSuite = serde_json::from_str(&raw_content)
            .unwrap_or_else(|_| panic!("Cannot parse suite: {:?}", test));
        for (_, unit) in parsed_suite.0 {
            run_test_unit(unit)
        }
    }
}
