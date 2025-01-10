//! Basing on <https://github.com/bluealloy/revm/blob/main/bins/revme/src/cmd/statetest/runner.rs>.
//! These tests may seem useless:
//! - They only have one transaction.
//! - REVM already tests them.
//!
//! Nevertheless, they are important:
//! - REVM doesn't test very tightly (not matching on expected failures, skipping tests, etc.).
//! - We must use a REVM fork (for distinguishing explicit & implicit reads, etc.).
//! - We use custom handlers (for lazy-updating the beneficiary account, etc.) that require "re-testing".
//! - Help outline the minimal state commitment logic for pevm.

use pevm::chain::PevmEthereum;
use pevm::{
    Bytecodes, ChainState, EvmAccount, EvmCode, InMemoryStorage, Pevm, PevmError,
    PevmTxExecutionResult,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use revm::db::PlainAccount;
use revm::primitives::ruint::ParseError;
use revm::primitives::{
    calc_excess_blob_gas, AccountInfo, BlobExcessGasAndPrice, BlockEnv, Bytecode, SpecId,
    TransactTo, TxEnv, KECCAK_EMPTY, U256,
};
use revme::cmd::statetest::models::{Env, SpecName, Test, TestSuite, TestUnit, TransactionParts};
use revme::cmd::statetest::{
    merkle_trie::{log_rlp_hash, state_merkle_trie_root},
    utils::recover_address,
};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::{fs, num::NonZeroUsize};
use walkdir::{DirEntry, WalkDir};

#[path = "../common/mod.rs"]
pub mod common;

fn build_block_env(env: &Env, spec_id: SpecId) -> BlockEnv {
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
            Some(BlobExcessGasAndPrice::new(
                current_excess_blob_gas.to(),
                spec_id.is_enabled_in(SpecId::PRAGUE),
            ))
        } else if let (Some(parent_blob_gas_used), Some(parent_excess_blob_gas)) =
            (env.parent_blob_gas_used, env.parent_excess_blob_gas)
        {
            Some(BlobExcessGasAndPrice::new(
                calc_excess_blob_gas(
                    parent_blob_gas_used.to(),
                    parent_excess_blob_gas.to(),
                    env.parent_target_blobs_per_block
                        .map(|i| i.to())
                        // https://github.com/bluealloy/revm/blob/a2451cdb30bd9d9aaca95f13bd50e2eafb619d8f/crates/specification/src/eip4844.rs#L23
                        .unwrap_or(3 * (1 << 17)),
                ),
                spec_id.is_enabled_in(SpecId::PRAGUE),
            ))
        } else {
            None
        },
    }
}

fn build_tx_env(path: &Path, tx: &TransactionParts, test: &Test) -> Result<TxEnv, ParseError> {
    Ok(TxEnv {
        caller: if let Some(address) = tx.sender {
            address
        } else if let Some(address) = recover_address(tx.secret_key.as_slice()) {
            address
        } else {
            panic!("Failed to parse caller for {path:?}");
        },
        gas_limit: tx.gas_limit[test.indexes.gas].saturating_to(),
        gas_price: tx.gas_price.or(tx.max_fee_per_gas).unwrap_or_default(),
        transact_to: match tx.to {
            Some(address) => TransactTo::Call(address),
            None => TransactTo::Create,
        },
        value: U256::from_str(&tx.value[test.indexes.value])?,
        data: tx.data[test.indexes.data].clone(),
        nonce: Some(tx.nonce.saturating_to()),
        chain_id: Some(1), // Ethereum mainnet
        access_list: tx
            .access_lists
            .get(test.indexes.data)
            .and_then(Option::as_deref)
            .cloned()
            .unwrap_or_default(),
        gas_priority_fee: tx.max_priority_fee_per_gas,
        blob_hashes: tx.blob_versioned_hashes.clone(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
        authorization_list: test.eip7702_authorization_list().unwrap(),
        #[cfg(feature = "optimism")]
        optimism: revm::primitives::OptimismFields::default(),
    })
}

fn run_test_unit(path: &Path, unit: TestUnit) {
    unit.post.into_par_iter().for_each(|(spec_name, tests)| {
        // Constantinople was immediately extended by Petersburg.
        // There was technically never a Constantinople transaction on mainnet
        // so REVM understandably doesn't support it (without Petersburg).
        if spec_name == SpecName::Constantinople {
            return;
        }

        tests.into_par_iter().for_each(|test| {
            let tx_env = build_tx_env(path, &unit.transaction, &test);
            if test.expect_exception.as_deref() == Some("TR_RLP_WRONGVALUE") && tx_env.is_err() {
                return;
            }

            let mut chain_state = ChainState::default();
            let mut bytecodes = Bytecodes::default();
            for (address, raw_info) in &unit.pre {
                let code = Bytecode::new_raw(raw_info.code.clone());
                let code_hash = if code.is_empty() {
                    None
                } else {
                    let code_hash = code.hash_slow();
                    bytecodes.insert(code_hash, EvmCode::from(code));
                    Some(code_hash)
                };
                chain_state.insert(
                    *address,
                    EvmAccount {
                        balance: raw_info.balance,
                        nonce: raw_info.nonce,
                        code_hash,
                        code: None,
                        storage: raw_info.storage.clone().into_iter().collect(),
                    },
                );
            }

            let spec_id = spec_name.to_spec_id();

            match (
                test.expect_exception.as_deref(),
                Pevm::default().execute_revm_parallel(
                    &PevmEthereum::mainnet(),
                    &InMemoryStorage::new(chain_state.clone(), Arc::new(bytecodes), Default::default()),
                    spec_id,
                    build_block_env(&unit.env, spec_id),
                    vec![tx_env.unwrap()],
                    NonZeroUsize::MIN,
                ),
            ) {
                // EIP-2681
                (Some("TR_NonceHasMaxValue"), Err(err)) => {
                    assert!(matches!(err, PevmError::NonceMismatch { .. }))
                }
                // Skipping special cases where REVM returns `Ok` on unsupported features.
                (Some("TR_TypeNotSupported"), Ok(_)) => {}
                // Remaining tests that expect execution to fail -> match error
                (Some(exception), Err(PevmError::ExecutionError(error))) => {
                    // TODO: Cleaner code would be nice..
                    assert!(match exception {
                        "TR_TypeNotSupported" => true, // REVM is yielding arbitrary errors in these cases.
                        "SenderNotEOA" => error == "Transaction(RejectCallerWithCode)",
                        "TR_NoFundsX" => error == "Transaction(OverflowPaymentInTransaction)",
                        "TransactionException.INSUFFICIENT_MAX_FEE_PER_BLOB_GAS" => error == "Transaction(BlobGasPriceGreaterThanMax)",
                        "TR_BLOBCREATE" => error == "Transaction(BlobCreateTransaction)",
                        "TR_GasLimitReached" => error == "Transaction(CallerGasLimitMoreThanBlock)",
                        "TR_TipGtFeeCap" => error == "Transaction(PriorityFeeGreaterThanMaxFee)",

                        "TR_NoFundsOrGas" | "IntrinsicGas" | "TR_IntrinsicGas" | "TransactionException.INTRINSIC_GAS_TOO_LOW" => error == "Transaction(CallGasCostMoreThanGasLimit)",
                        "TR_FeeCapLessThanBlocks" | "TransactionException.INSUFFICIENT_MAX_FEE_PER_GAS" => error == "Transaction(GasPriceLessThanBasefee)",
                        "TR_NoFunds" | "TransactionException.INSUFFICIENT_ACCOUNT_FUNDS" => &error[..31] == "Transaction(LackOfFundForMaxFee",
                        "TR_EMPTYBLOB" | "TransactionException.TYPE_3_TX_ZERO_BLOBS" => error == "Transaction(EmptyBlobs)",
                        "TR_BLOBLIST_OVERSIZE" | "TransactionException.TYPE_3_TX_BLOB_COUNT_EXCEEDED" => &error[..24] == "Transaction(TooManyBlobs",
                        "TR_BLOBVERSION_INVALID" | "TransactionException.TYPE_3_TX_INVALID_BLOB_VERSIONED_HASH" => error == "Transaction(BlobVersionNotSupported)",
                        "TransactionException.TYPE_3_TX_PRE_FORK|TransactionException.TYPE_3_TX_ZERO_BLOBS" | "TransactionException.TYPE_3_TX_PRE_FORK" => error == "Transaction(BlobVersionedHashesNotSupported)",
                        "TransactionException.INITCODE_SIZE_EXCEEDED" | "TR_InitCodeLimitExceeded" => error == "Transaction(CreateInitCodeSizeLimit)",
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
                    // pevm/revm to meet the Ethereum specs throughout the eras.
                    for (address, account) in state {
                        if let Some(account) = account {
                            let chain_state_account = chain_state.entry(address).or_default();
                            chain_state_account.balance = account.balance;
                            chain_state_account.nonce = account.nonce;
                            chain_state_account.code_hash = account.code_hash;
                            chain_state_account.code = account.code;
                            chain_state_account.storage.extend(account.storage.into_iter());
                        } else {
                            chain_state.remove(&address);
                        }
                    }
                    // TODO: Implement our own state root calculation function to remove
                    // this conversion to [PlainAccount]
                    let plain_chain_state = chain_state.into_iter().map(|(address, account)| {
                        (address, PlainAccount {
                            info: AccountInfo {
                                balance: account.balance,
                                nonce: account.nonce,
                                code_hash: account.code_hash.unwrap_or(KECCAK_EMPTY),
                                code: account.code.map(|evm_code| Bytecode::try_from(evm_code).unwrap()),
                            },
                            storage: account.storage.into_iter().collect(),
                        })}).collect::<Vec<_>>();
                    let state_root =
                        state_merkle_trie_root(plain_chain_state.iter().map(|(address, account)| (*address, account)));
                    assert_eq!(state_root, test.hash, "Mismatched state root for {path:?}");
                }
                unexpected_res => {
                    panic!("pevm doesn't match the test's expectation for {path:?}: {unexpected_res:?}")
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
        .par_iter()
        .for_each(|path| {
            let raw_content = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Cannot read suite {path:?}: {e:?}"));
            let TestSuite(suite) = serde_json::from_str(&raw_content)
                .unwrap_or_else(|e| panic!("Cannot parse suite {path:?}: {e:?}"));
            suite
                .into_par_iter()
                .for_each(|(_, unit)| run_test_unit(path, unit));
        });
}
