//! EIP-7702 regression tests.

use std::{num::NonZeroUsize, sync::Arc};

use pevm::{
    Bytecodes, ChainState, EvmAccount, EvmCode, InMemoryStorage, Pevm, chain::PevmEthereum,
    execute_revm_sequential,
};
use revm::{
    context::{
        BlockEnv, TransactionType, TxEnv,
        transaction::{Authorization, RecoveredAuthority, RecoveredAuthorization},
    },
    context_interface::either::Either,
    primitives::{Address, TxKind, U256, alloy_primitives::U160, hardfork::SpecId},
    state::Bytecode,
};

fn address(value: u64) -> Address {
    Address::from(U160::from(value))
}

fn delegate_tx(sponsor: Address, authority: Address, target: Address, nonce: u64) -> TxEnv {
    TxEnv {
        tx_type: TransactionType::Eip7702.into(),
        caller: sponsor,
        gas_limit: 100_000,
        gas_price: 1,
        kind: TxKind::Call(sponsor),
        nonce: 1,
        chain_id: Some(1),
        authorization_list: vec![Either::Right(RecoveredAuthorization::new_unchecked(
            Authorization {
                chain_id: U256::ZERO,
                address: target,
                nonce,
            },
            RecoveredAuthority::Valid(authority),
        ))],
        ..TxEnv::default()
    }
}

#[test]
fn redelegation_uses_latest_code() {
    let authority = address(1_000);
    let target_x = address(1_001);
    let target_y = address(1_002);
    let sponsor_x = address(2_000);
    let sponsor_y = address(2_001);
    let caller = address(2_002);

    let mut accounts = [sponsor_x, sponsor_y, caller]
        .into_iter()
        .map(|address| {
            (
                address,
                EvmAccount {
                    balance: U256::MAX / U256::from(2),
                    nonce: 1,
                    ..EvmAccount::default()
                },
            )
        })
        .collect::<ChainState>();
    accounts.insert(authority, EvmAccount::default());

    let mut bytecodes = Bytecodes::default();
    for (target, value) in [(target_x, 1), (target_y, 2)] {
        let bytecode = Bytecode::new_raw(vec![0x60, value, 0x60, 0x00, 0x55, 0x00].into());
        let code_hash = bytecode.hash_slow();
        let code = EvmCode::from(bytecode);
        accounts.insert(
            target,
            EvmAccount {
                nonce: 1,
                code_hash: Some(code_hash),
                code: Some(code.clone()),
                ..EvmAccount::default()
            },
        );
        bytecodes.insert(code_hash, code);
    }

    let txs = vec![
        delegate_tx(sponsor_x, authority, target_x, 0),
        delegate_tx(sponsor_y, authority, target_y, 1),
        TxEnv {
            caller,
            gas_limit: 100_000,
            gas_price: 1,
            kind: TxKind::Call(authority),
            nonce: 1,
            ..TxEnv::default()
        },
    ];

    let chain = PevmEthereum::mainnet();
    let storage = InMemoryStorage::new(accounts, Arc::new(bytecodes), Default::default());
    assert_eq!(
        execute_revm_sequential(
            &chain,
            &storage,
            SpecId::PRAGUE,
            BlockEnv::default(),
            txs.clone(),
        ),
        Pevm::default().execute_revm_parallel(
            &chain,
            &storage,
            SpecId::PRAGUE,
            BlockEnv::default(),
            txs,
            NonZeroUsize::MIN,
        ),
    );
}
