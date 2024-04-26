//! Run tests against BlockchainTests/

use revm::primitives::{Address, Bytes, B256, U256};
use revme::cmd::statetest::models as rmodels;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

pub(crate) use rmodels::{AccessList, AccountInfo};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockHeader {
    pub(crate) coinbase: Address,
    pub(crate) difficulty: U256,
    pub(crate) timestamp: U256,
    pub(crate) gas_limit: U256,
    pub(crate) number: U256,
    pub(crate) mix_hash: Option<B256>,
    pub(crate) base_fee_per_gas: Option<U256>,
    pub(crate) excess_blob_gas: Option<U256>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Block {
    pub(crate) block_header: BlockHeader,
    pub(crate) transactions: Vec<Transaction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transaction {
    pub(crate) gas_limit: Option<U256>,
    pub(crate) gas_price: Option<U256>,
    pub(crate) max_fee_per_gas: Option<U256>,
    pub(crate) max_priority_fee_per_gas: Option<U256>,
    pub(crate) max_fee_per_blob_gas: Option<U256>,
    pub(crate) sender: Address,
    pub(crate) to: Address,
    pub(crate) value: U256,
    pub(crate) data: Bytes,
    pub(crate) nonce: U256,
    pub(crate) chain_id: Option<U256>,
    pub(crate) access_list: Option<AccessList>,
    pub(crate) blob_versioned_hashes: Option<Vec<B256>>,
}

fn deserialize_str_as_spec_name<'de, D>(deserializer: D) -> Result<rmodels::SpecName, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    if string == "Paris" {
        Ok(rmodels::SpecName::Merge)
    } else {
        serde_json::from_value(serde_json::Value::String(string)) //
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockchainTestUnit {
    /// Test info is optional
    #[serde(default, rename = "_info")]
    pub(crate) info: Option<serde_json::Value>,
    pub(crate) pre: HashMap<Address, rmodels::AccountInfo>,
    pub(crate) blocks: Vec<Block>,
    #[serde(deserialize_with = "deserialize_str_as_spec_name")]
    pub(crate) network: rmodels::SpecName,
    pub(crate) post_state: HashMap<Address, rmodels::AccountInfo>,
}

#[derive(Debug, Error)]
pub(crate) enum BlockchainTestError {
    #[error(transparent)]
    StdIo(#[from] std::io::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

/// https://ethereum-tests.readthedocs.io/en/latest/test_types/blockchain_tests.html
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct BlockchainTestSuite(
    pub(crate) BTreeMap<String, BlockchainTestUnit>, //
);
