use std::{collections::HashMap, fmt::Display, sync::Arc};

use ahash::AHashMap;
use alloy_primitives::{Address, Bytes, B256, U256};
use bitvec::vec::BitVec;
use revm::{
    interpreter::analysis::to_analysed,
    primitives::{Account, AccountInfo, Bytecode, JumpTable, KECCAK_EMPTY},
    DatabaseRef,
};
use serde::{Deserialize, Serialize};

use crate::{BuildIdentityHasher, BuildSuffixHasher};

// TODO: Port EVM types to [primitives.rs] to focus solely
// on the [Storage] interface here.

/// An EVM account.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
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
    pub storage: AHashMap<U256, U256>,
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
#[derive(Debug, Clone, PartialEq)]
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

/// EVM Code, currently mapping to REVM's [ByteCode::LegacyAnalyzed].
// TODO: Support raw legacy & EOF
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EvmCode {
    /// Bytecode with 32 zero bytes padding
    bytecode: Bytes,
    /// Original bytes length
    original_len: usize,
    /// Jump table.
    jump_table: Arc<BitVec<u8>>,
}

impl From<EvmCode> for Bytecode {
    fn from(code: EvmCode) -> Self {
        // TODO: Better error handling.
        // A common trap would be converting a default [EvmCode] into
        // a [Bytecode]. On failure we should fallback to legacy and
        // analyse again.
        unsafe {
            Bytecode::new_analyzed(code.bytecode, code.original_len, JumpTable(code.jump_table))
        }
    }
}

impl From<Bytecode> for EvmCode {
    fn from(code: Bytecode) -> Self {
        match code {
            Bytecode::LegacyRaw(_) => to_analysed(code).into(),
            Bytecode::LegacyAnalyzed(code) => EvmCode {
                bytecode: code.bytecode,
                original_len: code.original_len,
                jump_table: code.jump_table.0,
            },
            Bytecode::Eof(_) => unimplemented!("TODO: Support EOF"),
            Bytecode::Eip7702(_) => unimplemented!("TODO: Support EIP-7702"),
        }
    }
}

/// Mapping from address to [EvmAccount]
pub type ChainState = HashMap<Address, EvmAccount, BuildSuffixHasher>;

/// Mapping from code hashes to [EvmCode]s
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

// We can use any REVM database as storage provider. Convenient for
// testing blocks fetched from RPC via REVM's [CachedDB]. Otherwise, use
// our [Storage] types to avoid redundant conversions.
// TODO: Do something equivalent to [CachedDB] ourselves and remove this.
impl<D: DatabaseRef> Storage for D
where
    D::Error: Display,
{
    type Error = D::Error;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        self.basic_ref(*address).map(|a| {
            a.map(|info| AccountBasic {
                balance: info.balance,
                nonce: info.nonce,
            })
        })
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        self.basic_ref(*address).map(|info| {
            info.and_then(|info| (!info.is_empty_code_hash()).then_some(info.code_hash))
        })
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        self.code_by_hash_ref(*code_hash).map(|bytecode| {
            if bytecode.is_empty() {
                None
            } else {
                Some(EvmCode::from(bytecode))
            }
        })
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        self.has_storage_ref(*address)
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        self.storage_ref(*address, *index)
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        self.block_hash_ref(*number)
    }
}

/// A Storage wrapper that implements REVM's [DatabaseRef], mainly used to
/// provide data for REVM's [CachedDB] for sequential fallback or via RPC.
#[derive(Debug)]
pub struct StorageWrapper<'a, S: Storage>(pub &'a S);

impl<'a, S: Storage> DatabaseRef for StorageWrapper<'a, S> {
    type Error = S::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(if let Some(basic) = self.0.basic(&address)? {
            let code_hash = self.0.code_hash(&address)?;
            let code = if let Some(code_hash) = &code_hash {
                self.0.code_by_hash(code_hash)?.map(Bytecode::from)
            } else {
                None
            };
            Some(AccountInfo {
                balance: basic.balance,
                nonce: basic.nonce,
                code_hash: code_hash.unwrap_or(KECCAK_EMPTY),
                code,
            })
        } else {
            None
        })
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0
            .code_by_hash(&code_hash)
            .map(|code| code.map(Bytecode::from).unwrap_or_default())
    }

    fn has_storage_ref(&self, address: Address) -> Result<bool, Self::Error> {
        self.0.has_storage(&address)
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
mod rpc;
pub use rpc::RpcStorage;
