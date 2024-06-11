// Basing on https://github.com/bluealloy/revm/blob/main/bins/revme/src/cmd/statetest/runner.rs.
// These tests may seem useless:
// - They only have one transaction.
// - REVM already tests them.
// Nevertheless, they are important:
// - REVM doesn't test very tightly (not matching on expected failures, skipping tests, etc.).
// - We must use a REVM fork (for distinguishing explicit & implicit reads, etc.).
// - We use custom handlers (for lazy-updating the beneficiary account, etc.) that require "re-testing".
// - Help outline the minimal state commitment logic for PEVM.

use ahash::AHashMap;
use pevm::{InMemoryStorage, PevmError, PevmTxExecutionResult};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use revm::db::PlainAccount;
use revm::primitives::ruint::ParseError;
use revm::primitives::{
    calc_excess_blob_gas, AccountInfo, BlobExcessGasAndPrice, BlockEnv, Bytecode, SpecId,
    TransactTo, TxEnv, U256,
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
use std::{fs, num::NonZeroUsize};
use walkdir::{DirEntry, WalkDir};

#[path = "../common/mod.rs"]
pub mod common;

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
    })
}

fn run_test_unit(path: &Path, unit: &TestUnit) {
    unit.post.par_iter().for_each(|(spec_name, tests)| {
        // Constantinople was immediately extended by Petersburg.
        // There was technically never a Constantinople transaction on mainnet
        // so REVM undestandably doesn't support it (without Petersburg).
        if *spec_name == SpecName::Constantinople {
            return;
        }
        let spec_id = spec_name.to_spec_id();

        tests.par_iter().for_each(|test| {
            let tx_env = build_tx_env(&unit.transaction, &test.indexes);
            if test.expect_exception.as_deref() == Some("TR_RLP_WRONGVALUE") && tx_env.is_err() {
                return;
            }

            let mut chain_state = AHashMap::new();
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
            }

            match (
                test.expect_exception.as_deref(),
                pevm::execute_revm(
                    InMemoryStorage::new(chain_state.clone(), []),
                    spec_id,
                    build_block_env(&unit.env),
                    vec![tx_env.unwrap()],
                    NonZeroUsize::MIN,
                ),
            ) {
                // EIP-2681
                (Some("TR_NonceHasMaxValue"), Ok(exec_results)) => {
                    assert!(exec_results.len() == 1);
                    assert!(exec_results[0].receipt.status.coerce_status());
                    // This is overly strict as we only need the newly created account's code to be empty.
                    // Extracting such account is unjustified complexity so let's live with this for now.
                    assert!(exec_results[0].state.values().all(|account| account
                        .as_ref()
                        .map_or(false, |account| account.basic.code.is_empty())));
                }
                // Skipping special cases where REVM returns `Ok` on unsupported features.
                (Some("TR_TypeNotSupported"), Ok(_)) => {}
                // Remaining tests that expect execution to fail -> match error
                (Some(exception), Err(PevmError::ExecutionError(error))) => {
                    // TODO: Cleaner code would be nice..
                    assert!(match exception {
                        "TR_TypeNotSupported" => true, // REVM is yielding arbitrary errors in these cases.
                        "SenderNotEOA" => error == "Transaction(RejectCallerWithCode)",
                        "TR_NoFunds" => error[..31].to_string() == "Transaction(LackOfFundForMaxFee",
                        "TR_NoFundsOrGas" => error == "Transaction(CallGasCostMoreThanGasLimit)",
                        "IntrinsicGas" => error == "Transaction(CallGasCostMoreThanGasLimit)",
                        "TR_NoFundsX" => error == "Transaction(OverflowPaymentInTransaction)",
                        "TR_IntrinsicGas" => error == "Transaction(CallGasCostMoreThanGasLimit)",
                        "TransactionException.INSUFFICIENT_MAX_FEE_PER_BLOB_GAS" => error == "Transaction(BlobGasPriceGreaterThanMax)",
                        "TR_FeeCapLessThanBlocks" => error == "Transaction(GasPriceLessThanBasefee)",
                        "TransactionException.INTRINSIC_GAS_TOO_LOW" => error == "Transaction(CallGasCostMoreThanGasLimit)",
                        "TR_BLOBLIST_OVERSIZE" => error[..24].to_string() == "Transaction(TooManyBlobs",
                        "TR_BLOBCREATE" => error == "Transaction(BlobCreateTransaction)",
                        "TransactionException.INITCODE_SIZE_EXCEEDED" => error == "Transaction(CreateInitCodeSizeLimit)",
                        "TransactionException.INSUFFICIENT_MAX_FEE_PER_GAS" => error == "Transaction(GasPriceLessThanBasefee)",
                        "TR_GasLimitReached" => error == "Transaction(CallerGasLimitMoreThanBlock)",
                        "TR_EMPTYBLOB" => error == "Transaction(EmptyBlobs)",
                        "TR_BLOBVERSION_INVALID" => error == "Transaction(BlobVersionNotSupported)",
                        "TransactionException.INSUFFICIENT_ACCOUNT_FUNDS" => error[..31].to_string() == "Transaction(LackOfFundForMaxFee",
                        "TransactionException.TYPE_3_TX_ZERO_BLOBS" => error == "Transaction(EmptyBlobs)",
                        "TransactionException.TYPE_3_TX_BLOB_COUNT_EXCEEDED" => error[..24].to_string() == "Transaction(TooManyBlobs",
                        "TR_TipGtFeeCap" => error == "Transaction(PriorityFeeGreaterThanMaxFee)",
                        "TransactionException.TYPE_3_TX_INVALID_BLOB_VERSIONED_HASH" => error == "Transaction(BlobVersionNotSupported)",
                        "TransactionException.TYPE_3_TX_PRE_FORK|TransactionException.TYPE_3_TX_ZERO_BLOBS" => error == "Transaction(MaxFeePerBlobGasNotSupported)",
                        "TransactionException.TYPE_3_TX_PRE_FORK" => error == "Transaction(BlobVersionedHashesNotSupported)",
                        "TR_InitCodeLimitExceeded" => error == "Transaction(CreateInitCodeSizeLimit)",
                        _ => panic!("Mismatched error!\nPath: {path:?}\nExpected: {exception:?}\nGot: {error:?}")
                    });
                }
                // Tests that exepect execution to succeed -> match post state root
                (None, Ok(exec_results)) => {
                    assert!(exec_results.len() == 1);
                    let PevmTxExecutionResult {receipt, state} = exec_results[0].clone();

                    let logs_root = log_rlp_hash(&receipt.logs);
                    assert_eq!(logs_root, test.logs, "Mismatched logs root for {path:?}");

                    // This is a good reference for a minimal state/DB commitment logic for
                    // PEVM/REVM to meet the Ethereum specs throughout the eras.
                    for (address, account) in state {
                        if let Some(account) = account {
                            if account.is_empty() && spec_id.is_enabled_in(SpecId::SPURIOUS_DRAGON) {
                                chain_state.remove(&address);
                            } else {
                                let chain_state_account = chain_state.entry(address).or_default();
                                chain_state_account.info = account.basic.into();
                                chain_state_account.storage.extend(account.storage.iter());
                            }
                        } else {
                            chain_state.remove(&address);
                        }
                    }
                    let state_root =
                        state_merkle_trie_root(chain_state.iter().map(|(k, v)| (*k, v)));
                    assert_eq!(state_root, test.hash, "Mismatched state root for {path:?}");
                }
                _ => {
                    panic!("PEVM doesn't match the test's expectation for {path:?}")
                }
            }
        });
    });
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
            suite
                .par_iter()
                .for_each(|(_, unit)| run_test_unit(path, unit));
        });
}
