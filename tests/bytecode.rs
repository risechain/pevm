use std::str::FromStr;

use alloy_primitives::{bytes, Address, Bytes};
use pevm::{Eip7702Code, EvmCode, LegacyCode};
use revm::{
    interpreter::analysis::to_analysed,
    primitives::{Bytecode, Eip7702Bytecode},
};

const BYTECODE: alloy_primitives::Bytes = bytes!("608060405234801561001057600080fd5b506004361061002b5760003560e01c8063920a769114610030575b600080fd5b61004361003e366004610374565b610055565b60405190815260200160405180910390f35b600061006082610067565b5192915050565b60606101e0565b818153600101919050565b600082840393505b838110156100a25782810151828201511860001a1590930292600101610081565b9392505050565b825b602082106100d75782516100c0601f8361006e565b5260209290920191601f19909101906021016100ab565b81156100a25782516100ec600184038361006e565b520160010192915050565b60006001830392505b61010782106101385761012a8360ff1661012560fd6101258760081c60e0018961006e565b61006e565b935061010682039150610100565b600782106101655761015e8360ff16610125600785036101258760081c60e0018961006e565b90506100a2565b61017e8360ff166101258560081c8560051b018761006e565b949350505050565b80516101d890838303906101bc90600081901a600182901a60081b1760029190911a60101b17639e3779b90260131c611fff1690565b8060021b6040510182815160e01c1860e01b8151188152505050565b600101919050565b5060405161800038823961800081016020830180600d8551820103826002015b81811015610313576000805b50508051604051600082901a600183901a60081b1760029290921a60101b91909117639e3779b9810260111c617ffc16909101805160e081811c878603811890911b9091189091528401908183039084841061026857506102a3565b600184019350611fff821161029d578251600081901a600182901a60081b1760029190911a60101b17810361029d57506102a3565b5061020c565b8383106102b1575050610313565b600183039250858311156102cf576102cc87878886036100a9565b96505b6102e3600985016003850160038501610079565b91506102f08782846100f7565b9650506103088461030386848601610186565b610186565b915050809350610200565b5050617fe061032884848589518601036100a9565b03925050506020820180820383525b81811161034e57617fe08101518152602001610337565b5060008152602001604052919050565b634e487b7160e01b600052604160045260246000fd5b60006020828403121561038657600080fd5b813567ffffffffffffffff8082111561039e57600080fd5b818401915084601f8301126103b257600080fd5b8135818111156103c4576103c461035e565b604051601f8201601f19908116603f011681019083821181831017156103ec576103ec61035e565b8160405282815287602084870101111561040557600080fd5b82602086016020830137600092810160200192909252509594505050505056fea264697066735822122000646b2953fc4a6f501bd0456ac52203089443937719e16b3190b7979c39511264736f6c63430008190033");

#[test]
fn test_evmcode_from_revm_bytecode_eip7702() {
    let addr = Address::new([0x01; 20]);

    // New from address.
    let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new(addr));
    let evmcode = EvmCode::from(bytecode);
    assert!(
        matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, .. })
            if delegated_address == addr
        )
    );

    // New fromn raw.
    let byte_str = format!("ef0100{}", addr.to_string().trim_start_matches("0x"));
    let raw = Bytes::from_str(&byte_str).unwrap();
    let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new_raw(raw).unwrap());
    let evmcode = EvmCode::from(bytecode);
    assert!(
        matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, version })
            if delegated_address == addr && version == 0
        )
    );
}

#[test]
fn test_evmcode_from_revm_bytecode_legacy_raw() {
    let contract_bytecode = Bytecode::new_legacy(BYTECODE);
    let analyzed = to_analysed(contract_bytecode.clone());
    // Create EvmCode from raw Bytecode.
    let evmcode = EvmCode::from(contract_bytecode);

    if let Bytecode::LegacyAnalyzed(legacy_analyzed) = analyzed {
        let raw_jump = legacy_analyzed.jump_table().0.clone();
        assert!(
            matches!(evmcode, EvmCode::Legacy(LegacyCode { bytecode, original_len, jump_table })
                if bytecode == *legacy_analyzed.bytecode() && original_len == legacy_analyzed.original_len() && jump_table == raw_jump
            )
        );
    } else {
        panic!("Expected LegacyAnalyzed Bytecode")
    }
}

#[test]
fn test_evmcode_from_revm_bytecode_legacy_analyzed() {
    let contract_bytecode = Bytecode::new_legacy(BYTECODE);
    let analyzed = to_analysed(contract_bytecode.clone());
    // Create EvmCode from analyzed bytecode.
    let evmcode = EvmCode::from(analyzed.clone());

    if let Bytecode::LegacyAnalyzed(legacy_analyzed) = analyzed {
        let raw_jump = legacy_analyzed.jump_table().0.clone();
        assert!(
            matches!(evmcode, EvmCode::Legacy(LegacyCode { bytecode, original_len, jump_table })
                if bytecode == *legacy_analyzed.bytecode() && original_len == legacy_analyzed.original_len() && jump_table == raw_jump
            )
        );
    } else {
        panic!("Expected LegacyAnalyzed Bytecode")
    }
}
