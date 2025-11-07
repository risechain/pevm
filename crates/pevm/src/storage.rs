use std::{fmt::Display, sync::Arc};

use alloy_primitives::{Address, B256, Bytes, U256};
use bitvec::vec::BitVec;
use hashbrown::HashMap;
use revm::{
    DatabaseRef,
    interpreter::analysis::to_analysed,
    primitives::{
        Account, AccountInfo, Bytecode, EIP7702_MAGIC_BYTES, Eip7702Bytecode, Eof, JumpTable,
        KECCAK_EMPTY,
    },
};
use rustc_hash::FxBuildHasher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    jump_table: Arc<BitVec<u8>>,
}

/// EIP7702 delegated code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eip7702Code {
    /// Address of the EOA which will inherit the bytecode.
    delegated_address: Address,
    /// Version of the bytecode.
    version: u8,
}

/// EVM Code, currently mapping to REVM's [`ByteCode`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvmCode {
    /// Maps both analyzed and non-analyzed REVM legacy bytecode.
    Legacy(LegacyCode),
    /// Maps delegated EIP7702 bytecode.
    Eip7702(Eip7702Code),
    /// Maps EOF bytecode.
    Eof(Bytes),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BytecodeConversionError {
    #[error("Failed to decode EOF")]
    EofDecodingError(#[source] revm::primitives::eof::EofDecodeError),
}

impl TryFrom<EvmCode> for Bytecode {
    type Error = BytecodeConversionError;
    fn try_from(code: EvmCode) -> Result<Self, Self::Error> {
        match code {
            // TODO: Turn this [unsafe] into a proper [Result]
            EvmCode::Legacy(code) => unsafe {
                Ok(Self::new_analyzed(
                    code.bytecode,
                    code.original_len,
                    JumpTable(code.jump_table),
                ))
            },
            EvmCode::Eip7702(code) => {
                let mut raw = EIP7702_MAGIC_BYTES.to_vec();
                raw.push(code.version);
                raw.extend(&code.delegated_address);
                Ok(Self::Eip7702(Eip7702Bytecode {
                    delegated_address: code.delegated_address,
                    version: code.version,
                    raw: raw.into(),
                }))
            }
            EvmCode::Eof(code) => Eof::decode(code)
                .map(Arc::new)
                .map(Self::Eof)
                .map_err(Self::Error::EofDecodingError),
        }
    }
}

impl From<Bytecode> for EvmCode {
    fn from(code: Bytecode) -> Self {
        match code {
            // This arm will recursively fallback to LegacyAnalyzed.
            Bytecode::LegacyRaw(_) => to_analysed(code).into(),
            Bytecode::LegacyAnalyzed(code) => Self::Legacy(LegacyCode {
                // We should be able to consume these instead of cloning if the
                // fields are `pub`. Won't be relevant as we rewrite `revm` anyway.
                bytecode: code.bytecode().clone(),
                original_len: code.original_len(),
                jump_table: code.jump_table().0.clone(),
            }),
            Bytecode::Eip7702(code) => Self::Eip7702(Eip7702Code {
                delegated_address: code.delegated_address,
                version: code.version,
            }),
            Bytecode::Eof(code) => Self::Eof(Arc::unwrap_or_clone(code).raw),
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
    type Error: Display;

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

/// revm [Database] errors when using pevm [Storage] as the underlying provider.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum StorageWrapperError<S: Storage> {
    #[error("storage error")]
    StorageError(S::Error),
    #[error("invalid byte code")]
    InvalidBytecode(BytecodeConversionError),
}

/// A Storage wrapper that implements REVM's [`DatabaseRef`] for ease of
/// integration.
#[derive(Debug)]
pub struct StorageWrapper<'a, S: Storage>(pub &'a S);

impl<S: Storage> DatabaseRef for StorageWrapper<'_, S> {
    type Error = StorageWrapperError<S>;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let Some(basic) = self
            .0
            .basic(&address)
            .map_err(StorageWrapperError::StorageError)?
        else {
            return Ok(None);
        };

        let code_hash = self
            .0
            .code_hash(&address)
            .map_err(StorageWrapperError::StorageError)?;

        let code = if let Some(hash) = &code_hash {
            self.0
                .code_by_hash(hash)
                .map_err(StorageWrapperError::StorageError)?
                .map(|c| Bytecode::try_from(c).map_err(StorageWrapperError::InvalidBytecode))
                .transpose()?
        } else {
            None
        };

        Ok(Some(AccountInfo {
            balance: basic.balance,
            nonce: basic.nonce,
            code_hash: code_hash.unwrap_or(KECCAK_EMPTY),
            code,
        }))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0
            .code_by_hash(&code_hash)
            .map_err(StorageWrapperError::StorageError)
            .and_then(|evm_code| {
                evm_code
                    .map(Bytecode::try_from)
                    .transpose()
                    .map(|bytecode: Option<Bytecode>| bytecode.unwrap_or_default())
                    .map_err(StorageWrapperError::InvalidBytecode)
            })
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0
            .storage(&address, &index)
            .map_err(StorageWrapperError::StorageError)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.0
            .block_hash(&number)
            .map_err(StorageWrapperError::StorageError)
    }
}

mod in_memory;
pub use in_memory::InMemoryStorage;
#[cfg(feature = "rpc-storage")]
mod rpc;
#[cfg(feature = "rpc-storage")]
pub use rpc::RpcStorage;

#[cfg(test)]
mod tests {
    use alloy_primitives::{Bytes, bytes};
    use revm::primitives::eip7702::EIP7702_VERSION;
    use revm::primitives::eof::EofDecodeError;

    use super::*;

    // Bytecode from Forge's default Counter.sol contract, compiled with solc 0.8.13.
    // https://github.com/foundry-rs/foundry/blob/nightly-fe2acca4e379793539db80e032d76ffe0110298b/testdata/multi-version/Counter.sol
    const BYTECODE: Bytes = bytes!(
        "608060405234801561001057600080fd5b5060f78061001f6000396000f3fe6080604052348015600f57600080fd5b5060043610603c5760003560e01c80633fb5c1cb1460415780638381f58a146053578063d09de08a14606d575b600080fd5b6051604c3660046083565b600055565b005b605b60005481565b60405190815260200160405180910390f35b6051600080549080607c83609b565b9190505550565b600060208284031215609457600080fd5b5035919050565b60006001820160ba57634e487b7160e01b600052601160045260246000fd5b506001019056fea264697066735822122012c25f3d90606133b37330bf079a425dbc650fd21060dee49f715d37d97cb58f64736f6c634300080d0033"
    );

    // Bytecode from revm test code.
    // https://github.com/bluealloy/revm/blob/925c042ad748695bc45e516dfd2457e7b44cd3a8/crates/bytecode/src/eof.rs#L210
    const EOF_BYTECODE: Bytes = bytes!("ef000101000402000100010400000000800000fe");

    fn eq_bytecodes(revm_code: &Bytecode, pevm_code: &EvmCode) -> bool {
        match (revm_code, pevm_code) {
            (Bytecode::LegacyAnalyzed(revm), EvmCode::Legacy(pevm)) => {
                revm.bytecode() == &pevm.bytecode
                    && revm.original_len() == pevm.original_len
                    && revm.jump_table().0 == pevm.jump_table
            }
            (Bytecode::Eip7702(revm), EvmCode::Eip7702(pevm)) => {
                revm.delegated_address == pevm.delegated_address && revm.version == pevm.version
            }
            (Bytecode::Eof(revm), EvmCode::Eof(pevm)) => revm.raw == pevm.0,
            _ => false,
        }
    }

    #[test]
    fn legacy_bytecodes() {
        let contract_bytecode = Bytecode::new_legacy(BYTECODE);
        let analyzed = to_analysed(contract_bytecode.clone());

        let evm_code = EvmCode::from(analyzed.clone());
        assert!(eq_bytecodes(&analyzed, &evm_code));
        assert_eq!(analyzed, evm_code.try_into().unwrap());

        let evm_code = EvmCode::from(contract_bytecode);
        assert!(eq_bytecodes(&analyzed, &evm_code));
        assert_eq!(analyzed, evm_code.try_into().unwrap());
    }

    #[test]
    fn eip7702_bytecodes() {
        let delegated_address = Address::new([0x01; 20]);

        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new(delegated_address));
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.try_into().unwrap());

        let mut bytes = EIP7702_MAGIC_BYTES.to_vec();
        bytes.push(EIP7702_VERSION);
        bytes.extend(delegated_address);
        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new_raw(bytes.into()).unwrap());
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.try_into().unwrap());

        let mut eip_bytecode = Eip7702Bytecode::new(delegated_address);
        // Mutate version and raw bytes after construction.
        let new_version = 5;
        eip_bytecode.version = new_version;
        let mut bytes = EIP7702_MAGIC_BYTES.to_vec();
        bytes.push(new_version);
        bytes.extend(delegated_address);
        eip_bytecode.raw = bytes.into();
        let bytecode = Bytecode::Eip7702(eip_bytecode);
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.try_into().unwrap());
    }

    #[test]
    fn eof_bytecodes() {
        let bytecode = Bytecode::Eof(Arc::new(Eof::decode(EOF_BYTECODE).unwrap()));
        let evm_code = EvmCode::from(bytecode.clone());
        assert!(eq_bytecodes(&bytecode, &evm_code));
        assert_eq!(bytecode, evm_code.try_into().unwrap());
    }

    #[test]
    fn eof_bytecodes_error() {
        assert_eq!(
            Bytecode::try_from(EvmCode::Eof(Bytes::new())),
            Err(BytecodeConversionError::EofDecodingError(
                EofDecodeError::MissingInput
            ))
        );

        let mut eof_dangling = EOF_BYTECODE.to_vec();
        eof_dangling.extend(bytes!("010203"));
        assert_eq!(
            Bytecode::try_from(EvmCode::Eof(eof_dangling.into())),
            Err(BytecodeConversionError::EofDecodingError(
                EofDecodeError::DanglingData
            ))
        );
    }
}
