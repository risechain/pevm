use std::str::FromStr;

use alloy_primitives::{Address, Bytes};
use pevm::{Eip7702Code, EvmCode};
use revm::primitives::{Bytecode, Eip7702Bytecode};

#[test]
fn test_evmcode_from_revm_bytecode_eip7702() {
    // From address
    let addr = Address::new([0x01; 20]);
    let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new(addr));
    let evmcode = EvmCode::from(bytecode);
    assert!(
        matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, .. })
            if delegated_address == addr
        )
    );

    // From raw
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
