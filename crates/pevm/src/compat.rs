// TODO: Support custom chains like OP & RISE
// Ideally REVM & Alloy would provide all these.

use alloy_primitives::U256;
use alloy_rpc_types_eth::Header;
use revm::{
    context::BlockEnv,
    context_interface::block::BlobExcessGasAndPrice,
    primitives::{
        eip4844::{BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE},
        hardfork::SpecId,
    },
};

/// Get the REVM block env of an Alloy block.
// https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L23-L48
// TODO: Better error handling & add tests, especially for [blob_excess_gas_and_price].
pub(crate) fn get_block_env(header: &Header, spec_id: impl Into<SpecId>) -> BlockEnv {
    let spec_id = spec_id.into();
    BlockEnv {
        number: U256::from(header.number),
        beneficiary: header.beneficiary,
        timestamp: U256::from(header.timestamp),
        gas_limit: header.gas_limit,
        basefee: header.base_fee_per_gas.unwrap_or_default(),
        difficulty: header.difficulty,
        prevrandao: Some(header.mix_hash),
        blob_excess_gas_and_price: header.excess_blob_gas.map(|excess_blob_gas| {
            BlobExcessGasAndPrice::new(
                excess_blob_gas,
                if spec_id.is_enabled_in(SpecId::PRAGUE) {
                    BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE
                } else {
                    BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN
                },
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256};

    fn test_header() -> Header {
        let mut header: Header = Header::default();
        header.number = 42;
        header.beneficiary = Address::from([0x11; 20]);
        header.timestamp = 1_717_171_717;
        header.gas_limit = 30_000_000;
        header.base_fee_per_gas = Some(123);
        header.difficulty = U256::from(456);
        header.mix_hash = B256::from([0x22; 32]);
        header
    }

    #[test]
    fn get_block_env_copies_basic_fields_and_keeps_blob_none() {
        let header = test_header();

        assert_eq!(
            get_block_env(&header, SpecId::CANCUN),
            BlockEnv {
                number: U256::from(header.number),
                beneficiary: header.beneficiary,
                timestamp: U256::from(header.timestamp),
                gas_limit: header.gas_limit,
                basefee: header.base_fee_per_gas.unwrap(),
                difficulty: header.difficulty,
                prevrandao: Some(header.mix_hash),
                blob_excess_gas_and_price: None,
            }
        );
    }

    #[test]
    fn get_block_env_uses_prague_blob_fraction_when_enabled() {
        let mut header = test_header();
        header.excess_blob_gas = Some(10_000_000);

        let cancun = BlobExcessGasAndPrice::new(
            header.excess_blob_gas.unwrap(),
            BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN,
        );
        let prague = BlobExcessGasAndPrice::new(
            header.excess_blob_gas.unwrap(),
            BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE,
        );

        assert_ne!(cancun, prague);
        assert_eq!(
            get_block_env(&header, SpecId::CANCUN).blob_excess_gas_and_price,
            Some(cancun),
        );
        assert_eq!(
            get_block_env(&header, SpecId::PRAGUE).blob_excess_gas_and_price,
            Some(prague),
        );
    }
}
