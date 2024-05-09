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
                        if account.is_empty() {
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
        "Cancun/stEIP1153-transientStorage/transStorageReset.json",
        "Cancun/stEIP5656-MCOPY/MCOPY_memory_expansion_cost.json",
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
        "stBadOpcode/invalidDiffPlaces.json",
        "stBadOpcode/opc0CDiffPlaces.json",
        "stBadOpcode/opc0DDiffPlaces.json",
        "stBadOpcode/opc0EDiffPlaces.json",
        "stBadOpcode/opc0FDiffPlaces.json",
        "stBadOpcode/opc1EDiffPlaces.json",
        "stBadOpcode/opc1FDiffPlaces.json",
        "stBadOpcode/opc21DiffPlaces.json",
        "stBadOpcode/opc22DiffPlaces.json",
        "stBadOpcode/opc23DiffPlaces.json",
        "stBadOpcode/opc24DiffPlaces.json",
        "stBadOpcode/opc25DiffPlaces.json",
        "stBadOpcode/opc26DiffPlaces.json",
        "stBadOpcode/opc27DiffPlaces.json",
        "stBadOpcode/opc28DiffPlaces.json",
        "stBadOpcode/opc29DiffPlaces.json",
        "stBadOpcode/opc2ADiffPlaces.json",
        "stBadOpcode/opc2BDiffPlaces.json",
        "stBadOpcode/opc2CDiffPlaces.json",
        "stBadOpcode/opc2DDiffPlaces.json",
        "stBadOpcode/opc2EDiffPlaces.json",
        "stBadOpcode/opc2FDiffPlaces.json",
        "stBadOpcode/opc49DiffPlaces.json",
        "stBadOpcode/opc4ADiffPlaces.json",
        "stBadOpcode/opc4BDiffPlaces.json",
        "stBadOpcode/opc4CDiffPlaces.json",
        "stBadOpcode/opc4DDiffPlaces.json",
        "stBadOpcode/opc4EDiffPlaces.json",
        "stBadOpcode/opc4FDiffPlaces.json",
        "stBadOpcode/opc5CDiffPlaces.json",
        "stBadOpcode/opc5DDiffPlaces.json",
        "stBadOpcode/opc5EDiffPlaces.json",
        "stBadOpcode/opc5FDiffPlaces.json",
        "stBadOpcode/opcA5DiffPlaces.json",
        "stBadOpcode/opcA6DiffPlaces.json",
        "stBadOpcode/opcA7DiffPlaces.json",
        "stBadOpcode/opcA8DiffPlaces.json",
        "stBadOpcode/opcA9DiffPlaces.json",
        "stBadOpcode/opcAADiffPlaces.json",
        "stBadOpcode/opcABDiffPlaces.json",
        "stBadOpcode/opcACDiffPlaces.json",
        "stBadOpcode/opcADDiffPlaces.json",
        "stBadOpcode/opcAEDiffPlaces.json",
        "stBadOpcode/opcAFDiffPlaces.json",
        "stBadOpcode/opcB0DiffPlaces.json",
        "stBadOpcode/opcB1DiffPlaces.json",
        "stBadOpcode/opcB2DiffPlaces.json",
        "stBadOpcode/opcB3DiffPlaces.json",
        "stBadOpcode/opcB4DiffPlaces.json",
        "stBadOpcode/opcB5DiffPlaces.json",
        "stBadOpcode/opcB6DiffPlaces.json",
        "stBadOpcode/opcB7DiffPlaces.json",
        "stBadOpcode/opcB8DiffPlaces.json",
        "stBadOpcode/opcB9DiffPlaces.json",
        "stBadOpcode/opcBADiffPlaces.json",
        "stBadOpcode/opcBBDiffPlaces.json",
        "stBadOpcode/opcBCDiffPlaces.json",
        "stBadOpcode/opcBDDiffPlaces.json",
        "stBadOpcode/opcBEDiffPlaces.json",
        "stBadOpcode/opcBFDiffPlaces.json",
        "stBadOpcode/opcC0DiffPlaces.json",
        "stBadOpcode/opcC1DiffPlaces.json",
        "stBadOpcode/opcC2DiffPlaces.json",
        "stBadOpcode/opcC3DiffPlaces.json",
        "stBadOpcode/opcC4DiffPlaces.json",
        "stBadOpcode/opcC5DiffPlaces.json",
        "stBadOpcode/opcC6DiffPlaces.json",
        "stBadOpcode/opcC7DiffPlaces.json",
        "stBadOpcode/opcC8DiffPlaces.json",
        "stBadOpcode/opcC9DiffPlaces.json",
        "stBadOpcode/opcCADiffPlaces.json",
        "stBadOpcode/opcCBDiffPlaces.json",
        "stBadOpcode/opcCCDiffPlaces.json",
        "stBadOpcode/opcCDDiffPlaces.json",
        "stBadOpcode/opcCEDiffPlaces.json",
        "stBadOpcode/opcCFDiffPlaces.json",
        "stBadOpcode/opcD0DiffPlaces.json",
        "stBadOpcode/opcD1DiffPlaces.json",
        "stBadOpcode/opcD2DiffPlaces.json",
        "stBadOpcode/opcD3DiffPlaces.json",
        "stBadOpcode/opcD4DiffPlaces.json",
        "stBadOpcode/opcD5DiffPlaces.json",
        "stBadOpcode/opcD6DiffPlaces.json",
        "stBadOpcode/opcD7DiffPlaces.json",
        "stBadOpcode/opcD8DiffPlaces.json",
        "stBadOpcode/opcD9DiffPlaces.json",
        "stBadOpcode/opcDADiffPlaces.json",
        "stBadOpcode/opcDBDiffPlaces.json",
        "stBadOpcode/opcDCDiffPlaces.json",
        "stBadOpcode/opcDDDiffPlaces.json",
        "stBadOpcode/opcDEDiffPlaces.json",
        "stBadOpcode/opcDFDiffPlaces.json",
        "stBadOpcode/opcE0DiffPlaces.json",
        "stBadOpcode/opcE1DiffPlaces.json",
        "stBadOpcode/opcE2DiffPlaces.json",
        "stBadOpcode/opcE3DiffPlaces.json",
        "stBadOpcode/opcE4DiffPlaces.json",
        "stBadOpcode/opcE5DiffPlaces.json",
        "stBadOpcode/opcE6DiffPlaces.json",
        "stBadOpcode/opcE7DiffPlaces.json",
        "stBadOpcode/opcE8DiffPlaces.json",
        "stBadOpcode/opcE9DiffPlaces.json",
        "stBadOpcode/opcEADiffPlaces.json",
        "stBadOpcode/opcEBDiffPlaces.json",
        "stBadOpcode/opcECDiffPlaces.json",
        "stBadOpcode/opcEDDiffPlaces.json",
        "stBadOpcode/opcEEDiffPlaces.json",
        "stBadOpcode/opcEFDiffPlaces.json",
        "stBadOpcode/opcF6DiffPlaces.json",
        "stBadOpcode/opcF7DiffPlaces.json",
        "stBadOpcode/opcF8DiffPlaces.json",
        "stBadOpcode/opcF9DiffPlaces.json",
        "stBadOpcode/opcFBDiffPlaces.json",
        "stBadOpcode/opcFCDiffPlaces.json",
        "stBadOpcode/opcFEDiffPlaces.json",
        "stCallCodes/call_OOG_additionalGasCosts2.json",
        "stCallCodes/touchAndGo.json",
        "stCallCreateCallCodeTest/createJS_ExampleContract.json",
        "stCreate2/CREATE2_HighNonceDelegatecall.json",
        "stCreate2/Create2OOGFromCallRefunds.json",
        "stCreate2/Create2OOGafterInitCodeReturndata.json",
        "stCreate2/Create2OOGafterInitCodeReturndata2.json",
        "stCreate2/Create2OOGafterInitCodeReturndata3.json",
        "stCreate2/RevertInCreateInInitCreate2.json",
        "stCreate2/RevertInCreateInInitCreate2Paris.json",
        "stCreate2/create2collisionStorage.json",
        "stCreate2/create2collisionStorageParis.json",
        "stCreate2/returndatacopy_following_create.json",
        "stCreate2/returndatacopy_following_successful_create.json",
        "stCreateTest/CreateOOGFromCallRefunds.json",
        "stCreateTest/CreateOOGFromEOARefunds.json",
        "stCreateTest/CreateResults.json",
        "stCreateTest/CreateTransactionHighNonce.json",
        "stEIP1559/baseFeeDiffPlaces.json",
        "stEIP1559/gasPriceDiffPlaces.json",
        "stEIP1559/lowGasLimit.json",
        "stEIP1559/typeTwoBerlin.json",
        "stEIP158Specific/callToEmptyThenCallError.json",
        "stEIP2930/variedContext.json",
        "stExample/basefeeExample.json",
        "stExample/eip1559.json",
        "stExtCodeHash/dynamicAccountOverwriteEmpty.json",
        "stExtCodeHash/dynamicAccountOverwriteEmpty_Paris.json",
        "stExtCodeHash/extCodeHashCALL.json",
        "stExtCodeHash/extCodeHashCALLCODE.json",
        "stExtCodeHash/extCodeHashDELEGATECALL.json",
        "stExtCodeHash/extCodeHashSTATICCALL.json",
        "stExtCodeHash/extCodeHashSelfInInit.json",
        "stExtCodeHash/extcodehashEmpty.json",
        "stExtCodeHash/extcodehashEmpty_Paris.json",
        "stMemoryTest/buffer.json",
        "stMemoryTest/bufferSrcOffset.json",
        "stNonZeroCallsTest/NonZeroValue_CALLCODE_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_CALL_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_CALL_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_DELEGATECALL_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_SUICIDE_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_SUICIDE_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALL_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALL_ToOneStorageKey_Paris.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALLwithData_ToOneStorageKey.json",
        "stNonZeroCallsTest/NonZeroValue_TransactionCALLwithData_ToOneStorageKey_Paris.json",
        "stPreCompiledContracts2/CallEcrecover_Overflow.json",
        "stRefundTest/refundResetFrontier.json",
        "stRefundTest/refund_CallA.json",
        "stRefundTest/refund_CallA_OOG.json",
        "stRefundTest/refund_CallA_notEnoughGasInCall.json",
        "stRefundTest/refund_CallToSuicideNoStorage.json",
        "stRefundTest/refund_CallToSuicideStorage.json",
        "stRefundTest/refund_CallToSuicideTwice.json",
        "stRefundTest/refund_OOG.json",
        "stRefundTest/refund_TxToSuicide.json",
        "stRefundTest/refund_TxToSuicideOOG.json",
        "stReturnDataTest/returndatacopy_after_failing_callcode.json",
        "stReturnDataTest/returndatacopy_after_failing_staticcall.json",
        "stReturnDataTest/returndatacopy_following_create.json",
        "stReturnDataTest/returndatacopy_following_failing_call.json",
        "stReturnDataTest/returndatacopy_following_successful_create.json",
        "stReturnDataTest/returndatacopy_following_too_big_transfer.json",
        "stReturnDataTest/returndatacopy_initial.json",
        "stReturnDataTest/returndatacopy_initial_256.json",
        "stReturnDataTest/returndatacopy_initial_big_sum.json",
        "stReturnDataTest/returndatacopy_overrun.json",
        "stReturnDataTest/tooLongReturnDataCopy.json",
        "stRevertTest/RevertInCreateInInit.json",
        "stRevertTest/RevertInCreateInInit_Paris.json",
        "stRevertTest/RevertOpcodeInCallsOnNonEmptyReturnData.json",
        "stRevertTest/RevertPrecompiledTouch.json",
        "stRevertTest/RevertPrecompiledTouchExactOOG.json",
        "stRevertTest/RevertPrecompiledTouch_storage.json",
        "stRevertTest/RevertPrefoundEmptyCall.json",
        "stRevertTest/RevertRemoteSubCallStorageOOG.json",
        "stRevertTest/TouchToEmptyAccountRevert2.json",
        "stRevertTest/TouchToEmptyAccountRevert3.json",
        "stSStoreTest/InitCollision.json",
        "stSStoreTest/InitCollisionParis.json",
        "stSStoreTest/sstore_Xto0.json",
        "stSStoreTest/sstore_Xto0to0.json",
        "stSStoreTest/sstore_Xto0toX.json",
        "stSStoreTest/sstore_Xto0toXto0.json",
        "stSStoreTest/sstore_Xto0toY.json",
        "stSStoreTest/sstore_XtoX.json",
        "stSStoreTest/sstore_XtoXto0.json",
        "stSStoreTest/sstore_XtoXtoX.json",
        "stSStoreTest/sstore_XtoXtoY.json",
        "stSStoreTest/sstore_XtoY.json",
        "stSStoreTest/sstore_XtoYto0.json",
        "stSStoreTest/sstore_XtoYtoX.json",
        "stSStoreTest/sstore_XtoYtoY.json",
        "stSStoreTest/sstore_XtoYtoZ.json",
        "stSStoreTest/sstore_changeFromExternalCallInInitCode.json",
        "stSStoreTest/sstore_gasLeft.json",
        "stSelfBalance/diffPlaces.json",
        "stSpecialTest/FailedCreateRevertsDeletionParis.json",
        "stSpecialTest/block504980.json",
        "stSpecialTest/eoaEmptyParis.json",
        "stSpecialTest/failed_tx_xcf416c53.json",
        "stStackTests/underflowTest.json",
        "stStaticCall/static_Call50000.json",
        "stStaticCall/static_InternlCallStoreClearsOOG.json",
        "stStaticCall/static_LoopCallsThenRevert.json",
        "stStaticCall/static_callBasic.json",
        "stStaticCall/static_callWithHighValueAndGasOOG.json",
        "stStaticCall/static_refund_CallA.json",
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
        "stTransactionTest/ContractStoreClearsOOG.json",
        "stTransactionTest/InternlCallStoreClearsOOG.json",
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
        "stZeroCallsRevert/ZeroValue_SUICIDE_ToOneStorageKey_OOGRevert.json",
        "stZeroCallsRevert/ZeroValue_SUICIDE_ToOneStorageKey_OOGRevert_Paris.json",
        "stZeroCallsTest/ZeroValue_CALLCODE_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_CALL_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_CALL_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_CALL_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_DELEGATECALL_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_SUICIDE_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_TransactionCALL_ToOneStorageKey_Paris.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToEmpty.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToOneStorageKey.json",
        "stZeroCallsTest/ZeroValue_TransactionCALLwithData_ToOneStorageKey_Paris.json",
        "stZeroKnowledge/ecmul_1-2_2_28000_96.json",
        "stZeroKnowledge/ecmul_1-2_340282366920938463463374607431768211456_28000_96.json",
        "stZeroKnowledge/ecmul_1-2_616_28000_96.json",
        "stZeroKnowledge/ecmul_1-2_9935_28000_96.json",
        "stZeroKnowledge/ecmul_1-2_9_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_64.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_80.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_80_Paris.json",
        "stZeroKnowledge/ecmul_1-3_0_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_1_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_1_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_2_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_2_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_340282366920938463463374607431768211456_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_340282366920938463463374607431768211456_28000_80.json",
        "stZeroKnowledge/ecmul_1-3_340282366920938463463374607431768211456_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_5616_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_5616_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_5617_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_5617_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_9935_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_9935_28000_96.json",
        "stZeroKnowledge/ecmul_1-3_9_28000_128.json",
        "stZeroKnowledge/ecmul_1-3_9_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_1456_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_1_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_2_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_5616_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_9935_28000_96.json",
        "stZeroKnowledge/ecmul_7827-6598_9_28000_96.json",
        "stZeroKnowledge/ecpairing_bad_length_191.json",
        "stZeroKnowledge/ecpairing_bad_length_193.json",
        "stZeroKnowledge/ecpairing_inputs.json",
        "stZeroKnowledge/ecpairing_one_point_fail.json",
        "stZeroKnowledge/ecpairing_one_point_insufficient_gas.json",
        "stZeroKnowledge/ecpairing_one_point_not_in_subgroup.json",
        "stZeroKnowledge/ecpairing_one_point_with_g1_zero.json",
        "stZeroKnowledge/ecpairing_one_point_with_g2_zero.json",
        "stZeroKnowledge/ecpairing_one_point_with_g2_zero_and_g1_invalid.json",
        "stZeroKnowledge/ecpairing_perturb_g2_by_curve_order.json",
        "stZeroKnowledge/ecpairing_perturb_g2_by_field_modulus.json",
        "stZeroKnowledge/ecpairing_perturb_g2_by_field_modulus_again.json",
        "stZeroKnowledge/ecpairing_perturb_g2_by_one.json",
        "stZeroKnowledge/ecpairing_perturb_zeropoint_by_curve_order.json",
        "stZeroKnowledge/ecpairing_perturb_zeropoint_by_field_modulus.json",
        "stZeroKnowledge/ecpairing_perturb_zeropoint_by_one.json",
        "stZeroKnowledge/ecpairing_three_point_fail_1.json",
        "stZeroKnowledge/ecpairing_three_point_match_1.json",
        "stZeroKnowledge/ecpairing_two_point_fail_1.json",
        "stZeroKnowledge/ecpairing_two_point_fail_2.json",
        "stZeroKnowledge/ecpairing_two_point_match_1.json",
        "stZeroKnowledge/ecpairing_two_point_match_2.json",
        "stZeroKnowledge/ecpairing_two_point_match_3.json",
        "stZeroKnowledge/ecpairing_two_point_match_4.json",
        "stZeroKnowledge/ecpairing_two_point_match_5.json",
        "stZeroKnowledge/ecpairing_two_point_oog.json",
        "stZeroKnowledge/ecpairing_two_points_with_one_g2_zero.json",
        "stZeroKnowledge2/ecadd_0-0_0-0_21000_80.json",
        "stZeroKnowledge2/ecadd_0-0_1-3_25000_128.json",
        "stZeroKnowledge2/ecadd_0-3_1-2_25000_128.json",
        "stZeroKnowledge2/ecadd_1-3_0-0_25000_80.json",
        "stZeroKnowledge2/ecadd_1-3_0-0_25000_80_Paris.json",
        "stZeroKnowledge2/ecadd_6-9_19274124-124124_25000_128.json",
        "stZeroKnowledge2/ecmul_0-0_2_28000_96.json",
        "stZeroKnowledge2/ecmul_0-0_340282366920938463463374607431768211456_28000_96.json",
        "stZeroKnowledge2/ecmul_0-0_5616_28000_96.json",
        "stZeroKnowledge2/ecmul_0-0_5617_28000_96.json",
        "stZeroKnowledge2/ecmul_0-0_9_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_0_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_0_28000_64.json",
        "stZeroKnowledge2/ecmul_0-3_0_28000_80.json",
        "stZeroKnowledge2/ecmul_0-3_0_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_1_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_1_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_2_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_2_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_340282366920938463463374607431768211456_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_340282366920938463463374607431768211456_28000_80.json",
        "stZeroKnowledge2/ecmul_0-3_340282366920938463463374607431768211456_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_5616_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_5616_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_5616_28000_96_Paris.json",
        "stZeroKnowledge2/ecmul_0-3_5617_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_5617_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_9935_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_9935_28000_96.json",
        "stZeroKnowledge2/ecmul_0-3_9_28000_128.json",
        "stZeroKnowledge2/ecmul_0-3_9_28000_96.json",
        "stZeroKnowledge2/ecmul_1-2_1_28000_96.json",
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
