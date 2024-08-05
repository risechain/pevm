use std::path::Path;

use alloy_primitives::{keccak256, Address, Bytes, FixedBytes, B256, B64, U256};
use libmdbx::{Database, DatabaseOptions, NoWriteMap};
use revm::primitives::Bytecode;

use super::{AccountBasic, EvmCode, Storage};

/// A storage that stores chain data in a MDBX database.
#[derive(Debug)]
pub struct OnDiskStorage {
    inner: Database<NoWriteMap>,
}

impl OnDiskStorage {
    /// Opens the on-disk storage at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, libmdbx::Error> {
        let db = Database::open_with_options(
            &path,
            DatabaseOptions {
                max_tables: Some(16),
                ..DatabaseOptions::default()
            },
        )?;
        Ok(Self { inner: db })
    }
}

impl Storage for OnDiskStorage {
    type Error = libmdbx::Error;

    fn basic(
        &self,
        address: &alloy_primitives::Address,
    ) -> Result<Option<super::AccountBasic>, Self::Error> {
        let tx = self.inner.begin_ro_txn()?;
        let Some(balance) = tx
            .open_table(Some("balance"))
            .and_then(|table| tx.get(&table, address.as_ref()))
            .map(|bytes: Option<[u8; 32]>| bytes.map(B256::from))?
        else {
            return Ok(None);
        };
        let Some(nonce) = tx
            .open_table(Some("nonce"))
            .and_then(|table| tx.get(&table, address.as_ref()))
            .map(|bytes: Option<[u8; 8]>| bytes.map(B64::from))?
        else {
            return Ok(None);
        };
        Ok(Some(AccountBasic {
            balance: balance.into(),
            nonce: nonce.into(),
        }))
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        let tx = self.inner.begin_ro_txn()?;
        let code_hash = tx
            .open_table(Some("code_hash"))
            .and_then(|table| tx.get(&table, address.as_ref()))
            .map(|bytes: Option<[u8; 32]>| bytes.map(B256::from))?;
        Ok(code_hash)
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        let tx = self.inner.begin_ro_txn()?;
        let Some(code) = tx
            .open_table(Some("code_by_hash"))
            .and_then(|table| tx.get(&table, code_hash.as_ref()))
            .map(|bytes: Option<Vec<u8>>| bytes.map(Bytes::from))?
        else {
            return Ok(None);
        };
        Ok(Some(EvmCode::from(Bytecode::new_raw(code))))
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        let tx = self.inner.begin_ro_txn()?;
        let has_storage = tx
            .open_table(Some("has_storage"))
            .and_then(|table| tx.get(&table, address.as_ref()))
            .map(|bytes: Option<()>| bytes.is_some())?;
        Ok(has_storage)
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        type B416 = FixedBytes<52>;
        let storage_key =
            B416::from_slice(&[address.as_slice(), B256::from(*index).as_slice()].concat());
        let tx = self.inner.begin_ro_txn()?;
        let Some(storage_value) = tx
            .open_table(Some("storage"))
            .and_then(|table| tx.get(&table, storage_key.as_ref()))
            .map(|bytes: Option<[u8; 32]>| bytes.map(B256::from))?
        else {
            return Ok(U256::ZERO);
        };
        Ok(storage_value.into())
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        let tx = self.inner.begin_ro_txn()?;
        let Some(block_hash) = tx
            .open_table(Some("block_hash"))
            .and_then(|table| tx.get(&table, B64::from(*number).as_ref()))
            .map(|bytes: Option<[u8; 32]>| bytes.map(B256::from))?
        else {
            return Ok(keccak256(number.to_string().as_bytes()));
        };
        Ok(block_hash)
    }
}
