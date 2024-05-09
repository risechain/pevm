// Basing this off REVM's bins/revme/src/cmd/statetest/runner.rs

use block_stm_revm::{BlockSTM, Storage};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use revm::db::PlainAccount;
use revm::primitives::{
    calc_excess_blob_gas, Account, AccountInfo, AccountStatus, Address, BlobExcessGasAndPrice,
    BlockEnv, Bytecode, EVMError, InvalidTransaction, ResultAndState, StorageSlot, TransactTo,
    TxEnv, U256,
};
use revme::cmd::statetest::models::{
    Env, SpecName, TestSuite, TestUnit, TransactionParts, TxPartIndices,
};
use revme::cmd::statetest::{
    merkle_trie::{log_rlp_hash, state_merkle_trie_root},
    utils::recover_address,
};
use std::path::Path;
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

fn build_tx_env(tx: &TransactionParts, indexes: &TxPartIndices) -> TxEnv {
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

fn run_test_unit(path: &Path, unit: TestUnit) {
    for (spec_name, tests) in unit.post {
        // Should REVM know and handle these better, or it is
        // truly fine to just skip them?
        if matches!(
            spec_name,
            SpecName::ByzantiumToConstantinopleAt5 | SpecName::Constantinople | SpecName::Unknown
        ) {
            continue;
        }
        let spec_id = spec_name.to_spec_id();

        for test in tests {
            // Ideally we only need an account representation for both cases
            // instead of using `PlainAccount` & `Account`. The former is used
            // simply to utilize REVM's test root calculation functions.
            let mut chain_state: HashMap<Address, PlainAccount> = HashMap::new();
            let mut block_stm_storage = Storage::default();

            // Shouldn't we parse accounts as `Account` instead of `AccountInfo`
            // to have initial storage states?
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
                test.expect_exception,
                BlockSTM::run(
                    block_stm_storage,
                    spec_id,
                    build_block_env(&unit.env),
                    vec![build_tx_env(&unit.transaction, &test.indexes)],
                    NonZeroUsize::MIN,
                ),
            ) {
                // Tests that expect execution to fail -> match error
                (Some(exception), Err(error)) => {
                    // TODO: Ideally the REVM errors would match the descriptive expectations more.
                    if exception != "TR_TypeNotSupported" && !matches!(
                        (exception.as_str(), &error),
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

                    for (address, account) in state {
                        if account.is_empty() || !account.is_touched() {
                            continue;
                        }
                        if account.is_selfdestructed() {
                            chain_state.remove(&address);
                        } else {
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

fn should_skip_test(path: &Path) -> bool {
    [
        // These tests are passed but time consuming, uncomment to skip them:
        // "stTimeConsuming/CALLBlake2f_MaxRounds.json",
        // "stTimeConsuming/static_Call50000_sha256.json",
        // "vmPerformance/loopMul.json",
        // "stQuadraticComplexityTest/Call50000_sha256.json",

        // Failing
        "Cancun/stEIP1153-transientStorage/03_tloadAfterStoreIs0.json",
        "Pyspecs/cancun/eip1153_tstore/tload_after_tstore_is_zero.json",
        "Pyspecs/cancun/eip6780_selfdestruct/create_selfdestruct_same_tx.json",
        "Pyspecs/cancun/eip6780_selfdestruct/delegatecall_from_new_contract_to_pre_existing_contract.json",
        "Pyspecs/cancun/eip6780_selfdestruct/delegatecall_from_pre_existing_contract_to_new_contract.json",
        "Pyspecs/cancun/eip6780_selfdestruct/self_destructing_initcode.json",
        "Pyspecs/cancun/eip6780_selfdestruct/self_destructing_initcode_create_tx.json",
        "Pyspecs/cancun/eip6780_selfdestruct/selfdestruct_pre_existing.json",
        "VMTests/vmIOandFlowOperations/jump.json",
        "VMTests/vmIOandFlowOperations/jumpi.json",
        "VMTests/vmIOandFlowOperations/loopsConditionals.json",
        "VMTests/vmIOandFlowOperations/mload.json",
        "VMTests/vmIOandFlowOperations/return.json",
        "VMTests/vmLogTest/log0.json",
        "VMTests/vmLogTest/log1.json",
        "VMTests/vmLogTest/log2.json",
        "VMTests/vmLogTest/log3.json",
        "VMTests/vmLogTest/log4.json",
        "stCallCodes/touchAndGo.json",
        "stCreate2/Create2OOGFromCallRefunds.json",
        "stCreate2/RevertInCreateInInitCreate2.json",
        "stCreate2/RevertInCreateInInitCreate2Paris.json",
        "stCreate2/create2collisionStorage.json",
        "stCreate2/create2collisionStorageParis.json",
        "stCreateTest/CreateOOGFromCallRefunds.json",
        "stCreateTest/CreateOOGFromEOARefunds.json",
        "stCreateTest/CreateResults.json",
        "stCreateTest/CreateTransactionHighNonce.json",
        "stEIP1559/typeTwoBerlin.json",
        "stEIP158Specific/callToEmptyThenCallError.json",
        "stExample/basefeeExample.json",
        "stExample/eip1559.json",
        "stExtCodeHash/dynamicAccountOverwriteEmpty.json",
        "stExtCodeHash/dynamicAccountOverwriteEmpty_Paris.json",
        "stExtCodeHash/extCodeHashCALL.json",
        "stExtCodeHash/extCodeHashSTATICCALL.json",
        "stExtCodeHash/extCodeHashSelfInInit.json",
        "stExtCodeHash/extcodehashEmpty.json",
        "stExtCodeHash/extcodehashEmpty_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_CALL_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_CALL_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_SUICIDE_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_SUICIDE_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALL_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALL_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALLwithData_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALLwithData_ToOneStorageKey_Paris.json",
        "stPreCompiledContracts2/CallEcrecover_Overflow.json",
        "stRefundTest/refundResetFrontier.json",
        "stRefundTest/refund_CallA.json",
        "stRefundTest/refund_CallA_notEnoughGasInCall.json",
        "stRefundTest/refund_CallToSuicideNoStorage.json",
        "stRefundTest/refund_CallToSuicideStorage.json",
        "stRefundTest/refund_CallToSuicideTwice.json",
        "stRefundTest/refund_TxToSuicide.json",
        "stRevertTest/RevertInCreateInInit.json",
        "stRevertTest/RevertInCreateInInit_Paris.json",
        "stRevertTest/RevertPrecompiledTouch.json",
        "stRevertTest/RevertPrecompiledTouchExactOOG.json",
        "stRevertTest/RevertPrecompiledTouch_storage.json",
        "stRevertTest/RevertPrefoundEmptyCall.json",
        "stRevertTest/TouchToEmptyAccountRevert2.json",
        "stRevertTest/TouchToEmptyAccountRevert3.json",
        "stSStoreTest/InitCollision.json",
        "stSStoreTest/InitCollisionParis.json",
        "stSpecialTest/block504980.json",
        "stSpecialTest/failed_tx_xcf416c53.json",
        "stStaticCall/static_refund_CallToSuicideNoStorage.json",
        "stStaticCall/static_refund_CallToSuicideTwice.json",
        "stSystemOperationsTest/doubleSelfdestructTouch.json",
        "stTimeConsuming/sstore_combinations_initial00.json",
        "stTimeConsuming/sstore_combinations_initial00_2.json",
        "stTimeConsuming/sstore_combinations_initial00_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial00_Paris.json",
        "stTimeConsuming/sstore_combinations_initial01.json",
        "stTimeConsuming/sstore_combinations_initial01_2.json",
        "stTimeConsuming/sstore_combinations_initial01_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial01_Paris.json",
        "stTimeConsuming/sstore_combinations_initial10.json",
        "stTimeConsuming/sstore_combinations_initial10_2.json",
        "stTimeConsuming/sstore_combinations_initial10_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial10_Paris.json",
        "stTimeConsuming/sstore_combinations_initial11.json",
        "stTimeConsuming/sstore_combinations_initial11_2.json",
        "stTimeConsuming/sstore_combinations_initial11_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial11_Paris.json",
        "stTimeConsuming/sstore_combinations_initial20.json",
        "stTimeConsuming/sstore_combinations_initial20_2.json",
        "stTimeConsuming/sstore_combinations_initial20_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial20_Paris.json",
        "stTimeConsuming/sstore_combinations_initial21.json",
        "stTimeConsuming/sstore_combinations_initial21_2.json",
        "stTimeConsuming/sstore_combinations_initial21_2_Paris.json",
        "stTimeConsuming/sstore_combinations_initial21_Paris.json",
        "stTransactionTest/StoreClearsAndInternlCallStoreClearsOOG.json",
        "stTransactionTest/StoreClearsAndInternlCallStoreClearsSuccess.json",
        "stTransactionTest/ValueOverflow.json",
        "stTransactionTest/ValueOverflowParis.json",
        "stWalletTest/dayLimitResetSpentToday.json",
        "stWalletTest/dayLimitSetDailyLimit.json",
        "stWalletTest/dayLimitSetDailyLimitNoData.json",
        "stWalletTest/multiOwnedAddOwner.json",
        "stWalletTest/multiOwnedAddOwnerAddMyself.json",
        "stWalletTest/multiOwnedChangeOwner.json",
        "stWalletTest/multiOwnedChangeOwnerNoArgument.json",
        "stWalletTest/multiOwnedChangeOwner_fromNotOwner.json",
        "stWalletTest/multiOwnedChangeOwner_toIsOwner.json",
        "stWalletTest/multiOwnedChangeRequirementTo0.json",
        "stWalletTest/multiOwnedChangeRequirementTo1.json",
        "stWalletTest/multiOwnedChangeRequirementTo2.json",
        "stWalletTest/multiOwnedIsOwnerFalse.json",
        "stWalletTest/multiOwnedIsOwnerTrue.json",
        "stWalletTest/multiOwnedRemoveOwnerByNonOwner.json",
        "stWalletTest/multiOwnedRemoveOwner_mySelf.json",
        "stWalletTest/multiOwnedRemoveOwner_ownerIsNotOwner.json",
        "stWalletTest/multiOwnedRevokeNothing.json",
        "stWalletTest/walletAddOwnerRemovePendingTransaction.json",
        "stWalletTest/walletChangeOwnerRemovePendingTransaction.json",
        "stWalletTest/walletChangeRequirementRemovePendingTransaction.json",
        "stWalletTest/walletConfirm.json",
        "stWalletTest/walletDefault.json",
        "stWalletTest/walletDefaultWithOutValue.json",
        "stWalletTest/walletExecuteOverDailyLimitMultiOwner.json",
        "stWalletTest/walletExecuteOverDailyLimitOnlyOneOwner.json",
        "stWalletTest/walletExecuteOverDailyLimitOnlyOneOwnerNew.json",
        "stWalletTest/walletExecuteUnderDailyLimit.json",
        "stWalletTest/walletKill.json",
        "stWalletTest/walletKillNotByOwner.json",
        "stWalletTest/walletKillToWallet.json",
        "stWalletTest/walletRemoveOwnerRemovePendingTransaction.json",
        "stZeroCallsTest/ZeroValue_CALL_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_CALL_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_CALL_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToOneStorageKey_Paris.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_80.json",
        "stZeroKnowledge/ecpairing_inputs.json",
        "stZeroKnowledge2/ecadd_0-0_0-0_21000_80.json",
        "stZeroKnowledge2/ecadd_1-3_0-0_25000_80.json",
        "stZeroKnowledge2/ecmul_0-3_5616_28000_96.json",
    ]
    .into_iter()
    .any(|test_name| path.ends_with(test_name))
}

#[test]
fn ethereum_tests() {
    WalkDir::new("tests/ethereum/tests/GeneralStateTests")
        .into_iter()
        .filter_map(Result::ok)
        .map(DirEntry::into_path)
        .filter(|path| path.extension() == Some("json".as_ref()))
        .filter(|path| !should_skip_test(path))
        .collect::<Vec<_>>()
        .par_iter() // TODO: Further improve test speed
        .for_each(|path| {
            let raw_content = fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Cannot read suite: {:?}", path));
            let TestSuite(suite) = serde_json::from_str(&raw_content)
                .unwrap_or_else(|_| panic!("Cannot parse suite: {:?}", path));
            for (_, unit) in suite {
                run_test_unit(path, unit)
            }
        });
}
