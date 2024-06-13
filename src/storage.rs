use std::fmt::Debug;

use ahash::AHashMap;
use alloy_primitives::{Address, Bytes, B256, U256};
use revm::{
    db::PlainAccount,
    primitives::{Account, AccountInfo, Bytecode},
    DatabaseRef,
};

/// An EVM account.
// TODO: Flatten `AccountBasic` or more ideally, replace this with an Alloy type.
// `AccountBasic` works for now as we're tightly tied to REVM types, hence
// conversions between `AccountBasic` & `AccountInfo` are very convenient.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EvmAccount {
    /// The account's basic information.
    pub basic: AccountBasic,
    /// The account's storage.
    pub storage: AHashMap<U256, U256>,
}

impl From<PlainAccount> for EvmAccount {
    fn from(account: PlainAccount) -> Self {
        EvmAccount {
            basic: account.info.into(),
            storage: account.storage.into_iter().collect(),
        }
    }
}

impl From<Account> for EvmAccount {
    fn from(account: Account) -> Self {
        Self {
            basic: account.info.into(),
            storage: account
                .storage
                .iter()
                .map(|(k, v)| (*k, v.present_value))
                .collect(),
        }
    }
}

/// Basic information of an account
// TODO: Reuse something sane from Alloy?
// TODO: More proper testing.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountBasic {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: u64,
    /// The code of the account.
    pub code: Bytes,
    /// The optional code hash to avoid rehashing during execution
    pub code_hash: Option<B256>,
}

impl Default for AccountBasic {
    fn default() -> Self {
        Self {
            balance: U256::ZERO,
            nonce: 0,
            code: Bytes::new(),
            code_hash: None,
        }
    }
}

impl From<AccountBasic> for AccountInfo {
    fn from(account: AccountBasic) -> Self {
        let code = Bytecode::new_raw(account.code);
        AccountInfo::new(
            account.balance,
            account.nonce,
            // TODO: try faster hashing with `asm-keccak`, `native-keccak`, etc.
            account.code_hash.unwrap_or_else(|| code.hash_slow()),
            code,
        )
    }
}

impl From<AccountInfo> for AccountBasic {
    fn from(account: AccountInfo) -> Self {
        AccountBasic {
            balance: account.balance,
            nonce: account.nonce,
            code: account.code.unwrap_or_default().original_bytes(),
            code_hash: Some(account.code_hash),
        }
    }
}

/// An interface to provide chain state to BlockSTM for transaction execution.
/// Staying close to the underlying REVM's Database trait while not leaking
/// its primitives to library users (favoring Alloy at the moment).
/// TODO: Better API for third-party integration.
pub trait Storage {
    /// Errors when querying data from storage.
    type Error: Debug;

    /// Get basic account information.
    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error>;

    /// Get account code by its hash.
    fn code_by_hash(&self, code_hash: &B256) -> Result<Bytes, Self::Error>;

    /// Get if the account already has storage (to support EIP-7610).
    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error>;

    /// Get storage value of address at index.
    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error>;

    /// Get block hash by block number.
    fn block_hash(&self, number: &U256) -> Result<B256, Self::Error>;
}

// We can use any REVM database as storage provider. Convenient for
// testing blocks fetched from RPC via REVM's CachedDB. Otherwise, use
// our `Storage` types to avoid redundant conversions.
// TODO: Do something equivalent to `CachedDB` ourselves and remove this.
impl<D: DatabaseRef> Storage for D
where
    D::Error: Debug,
{
    type Error = D::Error;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        D::basic_ref(self, *address).map(|a| a.map(|a| a.into()))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Bytes, Self::Error> {
        D::code_by_hash_ref(self, *code_hash).map(|b| b.original_bytes())
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        D::has_storage_ref(self, *address)
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        D::storage_ref(self, *address, *index)
    }

    fn block_hash(&self, number: &U256) -> Result<B256, Self::Error> {
        D::block_hash_ref(self, *number)
    }
}

// We want to use the Storage as REVM's DatabaseRef to provide data for
// things like sequential execution fallback.
pub(crate) struct StorageWrapper<S: Storage>(pub(crate) S);

impl<S: Storage> DatabaseRef for StorageWrapper<S> {
    type Error = S::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        S::basic(&self.0, &address).map(|account| account.map(AccountBasic::into))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        S::code_by_hash(&self.0, &code_hash).map(Bytecode::new_raw)
    }

    fn has_storage_ref(&self, address: Address) -> Result<bool, Self::Error> {
        S::has_storage(&self.0, &address)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        S::storage(&self.0, &address, &index)
    }

    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        S::block_hash(&self.0, &number)
    }
}

mod in_memory;
pub use in_memory::InMemoryStorage;
mod rpc;
pub use rpc::RpcStorage;
