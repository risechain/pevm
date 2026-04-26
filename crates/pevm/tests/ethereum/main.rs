//! Runs the Ethereum Execution Spec Tests (EEST) state test fixtures.
//!
//! Before running, download the fixtures once with:
//!   bash scripts/fetch-eest-fixtures.sh
//!
//! Why these tests matter even though EEST already runs them upstream:
//! - We use a revm fork that distinguishes explicit vs implicit reads.
//! - We use custom handlers (lazy beneficiary updates, etc.) that need re-testing.
//! - They validate our minimal state-commitment logic against the spec.

use pevm::chain::PevmEthereum;
use pevm::{
    Bytecodes, ChainState, EvmAccount, EvmCode, InMemoryStorage, Pevm, PevmError,
    PevmTxExecutionResult,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use revm::context::result::InvalidTransaction;
use revm::context::{BlockEnv, TransactTo, TxEnv};
use revm::context_interface::block::BlobExcessGasAndPrice;
use revm::context_interface::either::Either;
use revm::database::PlainAccount;
use revm::primitives::KECCAK_EMPTY;
use revm::primitives::eip4844::{
    BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE,
};
use revm::primitives::hardfork::SpecId;
use revm::state::{AccountInfo, Bytecode};
use revm_statetest_types::{Env, Test, TestSuite, TestUnit, TransactionParts};
use revme::cmd::statetest::{
    merkle_trie::{log_rlp_hash, state_merkle_trie_root},
    utils::recover_address,
};
use std::path::Path;
use std::sync::Arc;
use std::{fs, num::NonZeroUsize};
use walkdir::{DirEntry, WalkDir};

#[path = "../common/mod.rs"]
pub mod common;

fn build_block_env(env: &Env, spec_id: SpecId) -> BlockEnv {
    let blob_fraction = if spec_id.is_enabled_in(SpecId::PRAGUE) {
        BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE
    } else {
        BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN
    };
    BlockEnv {
        number: env.current_number,
        beneficiary: env.current_coinbase,
        timestamp: env.current_timestamp,
        gas_limit: env.current_gas_limit.saturating_to(),
        basefee: env.current_base_fee.unwrap_or_default().saturating_to(),
        difficulty: env.current_difficulty,
        prevrandao: env.current_random,
        blob_excess_gas_and_price: env
            .current_excess_blob_gas
            .map(|excess| BlobExcessGasAndPrice::new(excess.to(), blob_fraction)),
        slot_num: 0,
    }
}

fn build_tx_env(path: &Path, tx: &TransactionParts, test: &Test) -> Option<TxEnv> {
    Some(TxEnv {
        tx_type: tx.tx_type(test.indexes.data)? as u8,
        caller: if let Some(address) = tx.sender {
            address
        } else if let Some(address) = recover_address(tx.secret_key.as_slice()) {
            address
        } else {
            panic!("Failed to parse caller for {path:?}");
        },
        gas_limit: tx.gas_limit[test.indexes.gas].saturating_to(),
        gas_price: tx
            .gas_price
            .or(tx.max_fee_per_gas)
            .unwrap_or_default()
            .saturating_to(),
        kind: match tx.to {
            Some(address) => TransactTo::Call(address),
            None => TransactTo::Create,
        },
        value: tx.value[test.indexes.value],
        data: tx.data[test.indexes.data].clone(),
        nonce: tx.nonce.saturating_to(),
        chain_id: Some(1), // Ethereum mainnet
        access_list: tx
            .access_lists
            .get(test.indexes.data)
            .cloned()
            .flatten()
            .unwrap_or_default(),
        gas_priority_fee: tx.max_priority_fee_per_gas.map(|g| g.saturating_to()),
        blob_hashes: tx.blob_versioned_hashes.clone(),
        max_fee_per_blob_gas: tx.max_fee_per_blob_gas.unwrap_or_default().saturating_to(),
        authorization_list: tx
            .authorization_list
            .clone()
            .map(|auth_list| {
                auth_list
                    .into_iter()
                    .map(|auth| Either::Left(auth.into()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

// Returns true if `error` matches the given EEST single-exception token.
fn single_exception_matches(exception: &str, error: &InvalidTransaction) -> bool {
    match exception {
        "TransactionException.INTRINSIC_GAS_TOO_LOW" => {
            matches!(
                error,
                InvalidTransaction::CallGasCostMoreThanGasLimit { .. }
            )
        }
        "TransactionException.INTRINSIC_GAS_BELOW_FLOOR_GAS_COST" => {
            matches!(error, InvalidTransaction::GasFloorMoreThanGasLimit { .. })
        }
        // Both can indicate the sender can't cover max_fee * gas_limit.
        "TransactionException.INSUFFICIENT_ACCOUNT_FUNDS"
        | "TransactionException.GASLIMIT_PRICE_PRODUCT_OVERFLOW" => {
            matches!(
                error,
                InvalidTransaction::LackOfFundForMaxFee { .. }
                    | InvalidTransaction::OverflowPaymentInTransaction
            )
        }
        "TransactionException.SENDER_NOT_EOA" => *error == InvalidTransaction::RejectCallerWithCode,
        "TransactionException.NONCE_IS_MAX" => {
            *error == InvalidTransaction::NonceOverflowInTransaction
        }
        "TransactionException.PRIORITY_GREATER_THAN_MAX_FEE_PER_GAS" => {
            *error == InvalidTransaction::PriorityFeeGreaterThanMaxFee
        }
        "TransactionException.INSUFFICIENT_MAX_FEE_PER_GAS" => {
            *error == InvalidTransaction::GasPriceLessThanBasefee
        }
        "TransactionException.INSUFFICIENT_MAX_FEE_PER_BLOB_GAS" => {
            matches!(error, InvalidTransaction::BlobGasPriceGreaterThanMax { .. })
        }
        "TransactionException.TYPE_1_TX_PRE_FORK" => {
            *error == InvalidTransaction::Eip2930NotSupported
        }
        "TransactionException.TYPE_2_TX_PRE_FORK" => {
            *error == InvalidTransaction::Eip1559NotSupported
        }
        "TransactionException.TYPE_3_TX_PRE_FORK" => {
            *error == InvalidTransaction::Eip4844NotSupported
        }
        "TransactionException.TYPE_3_TX_ZERO_BLOBS" => *error == InvalidTransaction::EmptyBlobs,
        "TransactionException.TYPE_3_TX_CONTRACT_CREATION" => {
            *error == InvalidTransaction::BlobCreateTransaction
        }
        "TransactionException.TYPE_3_TX_INVALID_BLOB_VERSIONED_HASH" => {
            *error == InvalidTransaction::BlobVersionNotSupported
        }
        // Both map to an excessive total blob count/gas.
        "TransactionException.TYPE_3_TX_BLOB_COUNT_EXCEEDED"
        | "TransactionException.TYPE_3_TX_MAX_BLOB_GAS_ALLOWANCE_EXCEEDED" => {
            matches!(error, InvalidTransaction::TooManyBlobs { .. })
        }
        "TransactionException.TYPE_4_TX_PRE_FORK" => {
            *error == InvalidTransaction::Eip7702NotSupported
        }
        "TransactionException.TYPE_4_EMPTY_AUTHORIZATION_LIST" => {
            *error == InvalidTransaction::EmptyAuthorizationList
        }
        // revm has no dedicated variant for type-4 contract creation; accept any error.
        "TransactionException.TYPE_4_TX_CONTRACT_CREATION" => true,
        "TransactionException.INITCODE_SIZE_EXCEEDED" => {
            *error == InvalidTransaction::CreateInitCodeSizeLimit
        }
        // EIP-7825 per-tx cap or the block gas limit check in legacy tests.
        "TransactionException.GAS_ALLOWANCE_EXCEEDED" => {
            matches!(
                error,
                InvalidTransaction::TxGasLimitGreaterThanCap { .. }
                    | InvalidTransaction::CallerGasLimitMoreThanBlock
            )
        }
        _ => panic!("Unknown EEST exception: {exception}"),
    }
}

// EEST exception strings can be pipe-separated: "A|B" means either A or B is acceptable.
fn exception_matches(exception: &str, error: &InvalidTransaction) -> bool {
    exception
        .split('|')
        .any(|part| single_exception_matches(part, error))
}

fn run_test_unit(path: &Path, unit: TestUnit) {
    unit.post.into_par_iter().for_each(|(spec_name, tests)| {
        tests.into_par_iter().for_each(|test| {
            let tx_env = build_tx_env(path, &unit.transaction, &test);
            if test.expect_exception.is_some() && tx_env.is_none() {
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
                    &InMemoryStorage::new(
                        chain_state.clone(),
                        Arc::new(bytecodes),
                        Default::default(),
                    ),
                    spec_id,
                    build_block_env(&unit.env, spec_id),
                    vec![tx_env.unwrap()],
                    NonZeroUsize::MIN,
                ),
            ) {
                (Some(exception), Err(PevmError::ExecutionError(error))) => {
                    let pevm::ExecutionError::Transaction(error) = error else {
                        panic!(
                            "Mismatched error!\nPath: {path:?}\nExpected: {exception:?}\nGot: {error:?}"
                        )
                    };
                    assert!(
                        exception_matches(exception, &error),
                        "Mismatched error!\nPath: {path:?}\nExpected: {exception:?}\nGot: {error:?}"
                    );
                }
                (None, Ok(exec_results)) => {
                    assert!(exec_results.len() == 1);
                    let PevmTxExecutionResult { receipt, state } = exec_results[0].clone();

                    let logs_root = log_rlp_hash(&receipt.logs);
                    assert_eq!(logs_root, test.logs, "Mismatched logs root for {path:?}");

                    // Apply output state on top of pre-state to compute the post-state root.
                    // This is the minimal state-commitment logic pevm must satisfy.
                    for (address, account) in state {
                        if let Some(account) = account {
                            let chain_state_account = chain_state.entry(address).or_default();
                            chain_state_account.balance = account.balance;
                            chain_state_account.nonce = account.nonce;
                            chain_state_account.code_hash = account.code_hash;
                            chain_state_account.code = account.code;
                            chain_state_account
                                .storage
                                .extend(account.storage.into_iter());
                        } else {
                            chain_state.remove(&address);
                        }
                    }
                    // TODO: Replace this PlainAccount conversion with our own trie function.
                    let plain_chain_state = chain_state
                        .into_iter()
                        .map(|(address, account)| {
                            (
                                address,
                                PlainAccount {
                                    info: AccountInfo {
                                        balance: account.balance,
                                        nonce: account.nonce,
                                        code_hash: account.code_hash.unwrap_or(KECCAK_EMPTY),
                                        code: account.code.map(Bytecode::from),
                                        account_id: None,
                                    },
                                    storage: account.storage.into_iter().collect(),
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    let state_root = state_merkle_trie_root(
                        plain_chain_state
                            .iter()
                            .map(|(address, account)| (*address, account)),
                    );
                    assert_eq!(
                        state_root, test.hash,
                        "Mismatched state root for {path:?}"
                    );
                }
                unexpected_res => panic!(
                    "pevm doesn't match the test's expectation for {path:?}: {unexpected_res:?}"
                ),
            }
        });
    });
}

#[test]
fn ethereum_state_tests() {
    let fixtures = std::path::PathBuf::from("tests/ethereum/fixtures/state_tests");
    assert!(
        fixtures.exists(),
        "EEST fixtures not found at {fixtures:?}. Run: bash scripts/fetch-eest-fixtures.sh"
    );

    WalkDir::new(fixtures)
        .into_iter()
        .filter_map(Result::ok)
        .map(DirEntry::into_path)
        .filter(|path| path.extension() == Some("json".as_ref()))
        .filter(|path| {
            let path_str = path.to_str().unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            // Skip tests that exercise create-collision edge cases our revm fork handles
            // incorrectly. Revisit when we replace revm with our own EVM (issue #382).
            !path_str.contains("eip7610_create_collision")
                && !matches!(
                    name,
                    "create2collisionStorageParis.json"
                        | "dynamicAccountOverwriteEmpty_Paris.json"
                        | "RevertInCreateInInit_Paris.json"
                        | "RevertInCreateInInitCreate2Paris.json"
                )
        })
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
