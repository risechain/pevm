use std::error::Error as StdError;
use std::fmt::Debug;

use alloy_primitives::{Address, B256, Bytes, U256};
use hashbrown::HashMap;
use revm::{
    DatabaseRef,
    bytecode::{BytecodeKind, JumpTable},
    context::DBErrorMarker,
    primitives::KECCAK_EMPTY,
    state::{Account, AccountInfo, Bytecode},
};
use rustc_hash::FxBuildHasher;
use serde::{Deserialize, Serialize};

use crate::{BuildIdentityHasher, BuildSuffixHasher};

// TODO: Port EVM types to [primitives.rs] to focus solely
// on the [Storage] interface here.

/// An EVM account.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmAccount {
    /// The account's balance.
    pub balance: U256,
    /// The account's nonce.
    pub nonce: u64,
    /// The optional code hash of the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<B256>,
    /// The account's optional code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<EvmCode>,
    /// The account's storage.
    pub storage: HashMap<U256, U256, FxBuildHasher>,
}

impl From<Account> for EvmAccount {
    fn from(account: Account) -> Self {
        let has_code = !account.info.is_empty_code_hash();
        Self {
            balance: account.info.balance,
            nonce: account.info.nonce,
            code_hash: has_code.then_some(account.info.code_hash),
            code: has_code.then(|| account.info.code.unwrap().into()),
            storage: account
                .storage
                .into_iter()
                .map(|(k, v)| (k, v.present_value))
                .collect(),
        }
    }
}

/// Basic information of an account
// TODO: Reuse something sane from Alloy?
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBasic {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: u64,
}

impl Default for AccountBasic {
    fn default() -> Self {
        Self {
            balance: U256::ZERO,
            nonce: 0,
        }
    }
}

/// Analyzed legacy code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyCode {
    /// Bytecode with 32 zero bytes padding.
    // TODO: Store unpadded bytecode and pad on revm conversion
    bytecode: Bytes,
    /// Original bytes length.
    original_len: usize,
    /// Jump table.
    jump_table: JumpTable,
}

/// EVM Code, currently mapping to REVM's [`ByteCode`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvmCode {
    /// Maps both analyzed and non-analyzed REVM legacy bytecode.
    Legacy(LegacyCode),
    /// Maps delegated EIP7702 bytecode.
    Eip7702(Address),
}

impl From<EvmCode> for Bytecode {
    fn from(code: EvmCode) -> Self {
        match code {
            EvmCode::Legacy(code) => {
                Self::new_analyzed(code.bytecode, code.original_len, code.jump_table)
            }
            EvmCode::Eip7702(delegated_address) => Self::new_eip7702(delegated_address),
        }
    }
}

impl From<Bytecode> for EvmCode {
    fn from(code: Bytecode) -> Self {
        match code.kind() {
            BytecodeKind::LegacyAnalyzed => Self::Legacy(LegacyCode {
                bytecode: code.bytecode().clone(),
                original_len: code.len(),
                jump_table: code.legacy_jump_table().unwrap().clone(),
            }),
            BytecodeKind::Eip7702 => Self::Eip7702(code.eip7702_address().unwrap()),
        }
    }
}

/// Mapping from address to [`EvmAccount`]
pub type ChainState = HashMap<Address, EvmAccount, BuildSuffixHasher>;

/// Mapping from code hashes to [`EvmCode`]s
pub type Bytecodes = HashMap<B256, EvmCode, BuildSuffixHasher>;

/// Mapping from block numbers to block hashes
pub type BlockHashes = HashMap<u64, B256, BuildIdentityHasher>;

/// An interface to provide chain state to Pevm for transaction execution.
/// Staying close to the underlying REVM's Database trait while not leaking
/// its primitives to library users (favoring Alloy at the moment).
/// TODO: Better API for third-party integration.
pub trait Storage {
    /// Errors when querying data from storage.
    type Error: StdError + DBErrorMarker;

    /// Get basic account information.
    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error>;

    /// Get the code of an account.
    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error>;

    /// Get account code by its hash.
    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error>;

    /// Get if the account already has storage (to support EIP-7610).
    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error>;

    /// Get storage value of address at index.
    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error>;

    /// Get block hash by block number.
    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error>;
}

/// A Storage wrapper that implements REVM's [`DatabaseRef`] for ease of
/// integration.
#[derive(Debug)]
pub struct StorageWrapper<'a, S: Storage>(pub &'a S);

impl<S: Storage + Debug> DatabaseRef for StorageWrapper<'_, S> {
    type Error = S::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let Some(basic) = self.0.basic(&address)? else {
            return Ok(None);
        };

        let code_hash = self.0.code_hash(&address)?;

        let code = if let Some(hash) = &code_hash {
            self.0.code_by_hash(hash)?.map(Bytecode::from)
        } else {
            None
        };

        Ok(Some(AccountInfo {
            balance: basic.balance,
            nonce: basic.nonce,
            code_hash: code_hash.unwrap_or(KECCAK_EMPTY),
            code,
            account_id: None,
        }))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0
            .code_by_hash(&code_hash)
            .map(|evm_code| evm_code.map(Bytecode::from).unwrap_or_default())
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(&address, &index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.0.block_hash(&number)
    }
}

mod in_memory;
pub use in_memory::InMemoryStorage;
#[cfg(feature = "rpc-storage")]
mod rpc;
#[cfg(feature = "rpc-storage")]
pub use rpc::{RpcStorage, RpcStorageError};

#[cfg(test)]
mod tests {
    use alloy_primitives::{Bytes, bytes};

    use super::*;

    // Bytecode from Forge's default Counter.sol contract, compiled with solc 0.8.13.
    // https://github.com/foundry-rs/foundry/blob/nightly-fe2acca4e379793539db80e032d76ffe0110298b/testdata/multi-version/Counter.sol
    const BYTECODE: Bytes = bytes!(
        "608060405234801561001057600080fd5b5060f78061001f6000396000f3fe6080604052348015600f57600080fd5b5060043610603c5760003560e01c80633fb5c1cb1460415780638381f58a146053578063d09de08a14606d575b600080fd5b6051604c3660046083565b600055565b005b605b60005481565b60405190815260200160405180910390f35b6051600080549080607c83609b565b9190505550565b600060208284031215609457600080fd5b5035919050565b60006001820160ba57634e487b7160e01b600052601160045260246000fd5b506001019056fea264697066735822122012c25f3d90606133b37330bf079a425dbc650fd21060dee49f715d37d97cb58f64736f6c634300080d0033"
    );

    fn eq_bytecodes(revm_code: &Bytecode, pevm_code: &EvmCode) -> bool {
        match (revm_code.kind(), pevm_code) {
            (BytecodeKind::LegacyAnalyzed, EvmCode::Legacy(pevm)) => {
                revm_code.bytecode() == &pevm.bytecode
                    && revm_code.len() == pevm.original_len
                    && revm_code.legacy_jump_table().unwrap() == &pevm.jump_table
            }
            (BytecodeKind::Eip7702, EvmCode::Eip7702(pevm_address)) => {
                revm_code.eip7702_address().unwrap() == *pevm_address
            }
            _ => false,
        }
    }

    #[test]
    fn legacy_bytecodes() {
        let bytecode = Bytecode::new_legacy(BYTECODE);
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.into());
    }

    #[test]
    fn eip7702_bytecodes() {
        let delegated_address = Address::new([0x01; 20]);
        let bytecode = Bytecode::new_eip7702(delegated_address);
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.into());
    }
}
