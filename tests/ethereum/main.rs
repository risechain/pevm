// Basing on REVM's bins/revme/src/cmd/statetest/runner.rs
// https://github.com/bluealloy/revm/blob/main/bins/revme/src/cmd/statetest/runner.rs

use block_stm_revm::{BlockSTM, Storage};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use revm::db::PlainAccount;
use revm::primitives::ruint::ParseError;
use revm::primitives::{
    calc_excess_blob_gas, Account, AccountInfo, AccountStatus, Address, BlobExcessGasAndPrice,
    BlockEnv, Bytecode, Bytes, EVMError, ExecutionResult, HaltReason, InvalidTransaction, Output,
    ResultAndState, SpecId, StorageSlot, SuccessReason, TransactTo, TxEnv, U256,
};
use revme::cmd::statetest::models::{
    Env, SpecName, TestSuite, TestUnit, TransactionParts, TxPartIndices,
};
use revme::cmd::statetest::{
    merkle_trie::{log_rlp_hash, state_merkle_trie_root},
    utils::recover_address,
};
use std::path::Path;
use std::str::FromStr;
use std::{collections::HashMap, fs, num::NonZeroUsize};
use walkdir::{DirEntry, WalkDir};

fn build_block_env(env: &Env) -> BlockEnv {
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

fn build_tx_env(tx: &TransactionParts, indexes: &TxPartIndices) -> Result<TxEnv, ParseError> {
    Ok(TxEnv {
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
        value: U256::from_str(&tx.value[indexes.value])?,
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
    })
}

fn run_test_unit(path: &Path, unit: TestUnit) {
    for (spec_name, tests) in unit.post {
        // TODO: Should REVM know and handle these better, or it is
        // truly fine to just skip them?
        if matches!(
            spec_name,
            SpecName::ByzantiumToConstantinopleAt5 | SpecName::Constantinople | SpecName::Unknown
        ) {
            continue;
        }
        let spec_id = spec_name.to_spec_id();

        for test in tests {
            let tx_env = build_tx_env(&unit.transaction, &test.indexes);
            if test.expect_exception.as_deref() == Some("TR_RLP_WRONGVALUE") && tx_env.is_err() {
                continue;
            }

            // Ideally we only need an account representation for both cases
            // instead of using `PlainAccount` & `Account`. The former is used
            // simply to utilize REVM's test root calculation functions.
            let mut chain_state: HashMap<Address, PlainAccount> = HashMap::new();
            let mut block_stm_storage = Storage::default();

            for (address, raw_info) in unit.pre.iter() {
                let code = Bytecode::new_raw(raw_info.code.clone());
                let info =
                    AccountInfo::new(raw_info.balance, raw_info.nonce, code.hash_slow(), code);
                chain_state.insert(
                    *address,
                    PlainAccount {
                        info: info.clone(),
                        storage: raw_info.storage.clone(),
                    },
                );
                block_stm_storage.insert_account(
                    *address,
                    Account {
                        info,
                        storage: raw_info
                            .storage
                            .iter()
                            .map(|(key, value)| (*key, StorageSlot::new(*value)))
                            .collect(),
                        status: AccountStatus::Loaded,
                    },
                );
            }

            match (
                test.expect_exception.as_deref(),
                BlockSTM::run(
                    block_stm_storage,
                    spec_id,
                    build_block_env(&unit.env),
                    vec![tx_env.unwrap()],
                    NonZeroUsize::MIN,
                ),
            ) {
                // EIP-2681
                (Some("TR_NonceHasMaxValue"), Ok(exec_results)) => {
                    // TODO: We really should test with blocks with more than 1 tx
                    assert!(exec_results.len() == 1);
                    assert!(match exec_results[0].result.clone() {
                        ExecutionResult::Success {
                            output: Output::Create(b, None),
                            ..
                        } => b == Bytes::new(),
                        _ => false,
                    });
                }
                // Special cases where REVM returns `Ok` instead of `Err` on unsupported features.
                // Requiring stopping or halting reasons for now.
                (Some("TR_TypeNotSupported"), Ok(exec_results)) => {
                    // TODO: We really should test with blocks with more than 1 tx
                    assert!(exec_results.len() == 1);
                    assert!(matches!(
                        exec_results[0].result,
                        ExecutionResult::Halt {
                            reason: HaltReason::NotActivated,
                            ..
                        } | ExecutionResult::Success {
                            reason: SuccessReason::Stop,
                            ..
                        }
                    ));
                }
                // Remaining tests that expect execution to fail -> match error
                (Some(exception), Err(error)) => {
                    // TODO: Ideally the REVM errors would match the descriptive expectations more.
                    if exception != "TR_TypeNotSupported" && !matches!(
                        (exception, &error),
                        (
                            "TR_BLOBLIST_OVERSIZE",
                            EVMError::Transaction(InvalidTransaction::TooManyBlobs{..})
                        ) | (
                            "TR_BLOBCREATE",
                            EVMError::Transaction(InvalidTransaction::BlobCreateTransaction)
                        ) | (
                            "TR_EMPTYBLOB",
                            EVMError::Transaction(InvalidTransaction::EmptyBlobs)
                        ) | (
                            "TR_BLOBVERSION_INVALID",
                            EVMError::Transaction(InvalidTransaction::BlobVersionNotSupported)
                        ) | (
                            "TransactionException.TYPE_3_TX_PRE_FORK|TransactionException.TYPE_3_TX_ZERO_BLOBS",
                            EVMError::Transaction(InvalidTransaction::MaxFeePerBlobGasNotSupported)
                        ) | (
                            "TransactionException.TYPE_3_TX_PRE_FORK",
                            EVMError::Transaction(InvalidTransaction::BlobVersionedHashesNotSupported)
                        ) | (
                            "TransactionException.INSUFFICIENT_ACCOUNT_FUNDS",
                            EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee{..})
                        ) | (
                            "TransactionException.TYPE_3_TX_INVALID_BLOB_VERSIONED_HASH",
                            EVMError::Transaction(InvalidTransaction::BlobVersionNotSupported)
                        ) | (
                            "TransactionException.INSUFFICIENT_MAX_FEE_PER_GAS",
                            EVMError::Transaction(InvalidTransaction::GasPriceLessThanBasefee)
                        ) | (
                            "TransactionException.TYPE_3_TX_ZERO_BLOBS",
                            EVMError::Transaction(InvalidTransaction::EmptyBlobs)
                        ) | (
                            "TransactionException.TYPE_3_TX_BLOB_COUNT_EXCEEDED",
                            EVMError::Transaction(InvalidTransaction::TooManyBlobs{..})
                        ) | (
                            "TransactionException.INSUFFICIENT_MAX_FEE_PER_BLOB_GAS",
                            EVMError::Transaction(InvalidTransaction::BlobGasPriceGreaterThanMax)
                        ) | (
                            "TransactionException.INITCODE_SIZE_EXCEEDED",
                            EVMError::Transaction(InvalidTransaction::CreateInitCodeSizeLimit)
                        ) | (
                            "TransactionException.INTRINSIC_GAS_TOO_LOW",
                            EVMError::Transaction(InvalidTransaction::CallGasCostMoreThanGasLimit)
                        ) | (
                            "TR_InitCodeLimitExceeded",
                            EVMError::Transaction(InvalidTransaction::CreateInitCodeSizeLimit)
                        ) | (
                            "TR_IntrinsicGas",
                            EVMError::Transaction(InvalidTransaction::CallGasCostMoreThanGasLimit)
                        ) | (
                            "TR_FeeCapLessThanBlocks",
                            EVMError::Transaction(InvalidTransaction::GasPriceLessThanBasefee)
                        ) | (
                            "TR_NoFunds",
                            EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee{..})
                        ) | (
                            "TR_TipGtFeeCap",
                            EVMError::Transaction(InvalidTransaction::PriorityFeeGreaterThanMaxFee)
                        ) | (
                            "SenderNotEOA",
                            EVMError::Transaction(InvalidTransaction::RejectCallerWithCode)
                        ) | (
                            "TR_NoFundsX",
                            EVMError::Transaction(InvalidTransaction::OverflowPaymentInTransaction)
                        ) | (
                            "TR_NoFundsOrGas",
                            EVMError::Transaction(InvalidTransaction::CallGasCostMoreThanGasLimit)
                        ) | (
                            "IntrinsicGas",
                            EVMError::Transaction(InvalidTransaction::CallGasCostMoreThanGasLimit)
                        ) | (
                            "TR_GasLimitReached",
                            EVMError::Transaction(InvalidTransaction::CallerGasLimitMoreThanBlock)
                        )
                    ) {
                        panic!("Mismatched error!\nPath: {path:?}\nExpected: {exception:?}\nGot: {error:?}");
                    }
                }
                // Tests that exepect execution to succeed -> match post state root
                (None, Ok(exec_results)) => {
                    // TODO: We really should test with blocks with more than 1 tx
                    assert!(exec_results.len() == 1);
                    let ResultAndState { result, state } = exec_results[0].clone();

                    let logs_root = log_rlp_hash(result.logs());
                    assert_eq!(logs_root, test.logs, "Mismatched logs root for {path:?}");

                    // This is a good reference for a minimal state/DB commitment logic for
                    // BlockSTM/REVM to meet the Ethereum specs throughout the eras.
                    for (address, account) in state {
                        if !account.is_touched() {
                            continue;
                        }
                        if account.is_selfdestructed()
                            || (account.is_empty()
                                && spec_id.is_enabled_in(SpecId::SPURIOUS_DRAGON))
                        {
                            chain_state.remove(&address);
                            continue;
                        }
                        let chain_state_account = chain_state.entry(address).or_default();
                        chain_state_account.info = account.info;
                        chain_state_account
                            .storage
                            .extend(account.storage.iter().map(|(k, v)| (*k, v.present_value)));
                    }

                    let state_root =
                        state_merkle_trie_root(chain_state.iter().map(|(k, v)| (*k, v)));
                    assert_eq!(state_root, test.hash, "Mismatched state root for {path:?}");
                }
                _ => {
                    panic!("BlockSTM doesn't match the test's expectation for {path:?}")
                }
            }
        }
    }
}

#[test]
fn ethereum_state_tests() {
    WalkDir::new("tests/ethereum/tests/GeneralStateTests")
        .into_iter()
        .filter_map(Result::ok)
        .map(DirEntry::into_path)
        .filter(|path| path.extension() == Some("json".as_ref()))
        // For development, we can further filter to run a small set of tests,
        // or filter out time-consuming tests like:
        //   - stTimeConsuming/**
        //   - vmPerformance/loopMul.json
        //   - stQuadraticComplexityTest/Call50000_sha256.json
        .collect::<Vec<_>>()
        .par_iter() // TODO: Further improve test speed
        .for_each(|path| {
            let raw_content = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Cannot read suite {path:?}: {e:?}"));
            let TestSuite(suite) = serde_json::from_str(&raw_content)
                .unwrap_or_else(|e| panic!("Cannot parse suite {path:?}: {e:?}"));
            for (_, unit) in suite {
                run_test_unit(path, unit)
            }
        });
}
