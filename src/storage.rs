use std::{fmt::Debug, future::IntoFuture};

use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::{BlockId, BlockNumberOrTag};
use alloy_transport::TransportError;
use alloy_transport_http::Http;
use reqwest::Client;
use revm::{
    primitives::{AccountInfo, Bytecode},
    DatabaseRef,
};
use tokio::runtime::Runtime;

/// Basic information of an account
// TODO: Reuse something sane from Alloy?
// TODO: More proper testing.
#[derive(Debug)]
pub struct AccountBasic {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: u64,
    /// The code of the account.
    pub code: Bytes,
}

impl Default for AccountBasic {
    fn default() -> Self {
        Self {
            balance: U256::ZERO,
            nonce: 0,
            code: Bytes::new(),
        }
    }
}

impl From<AccountBasic> for AccountInfo {
    fn from(account: AccountBasic) -> Self {
        let code = Bytecode::new_raw(account.code);
        AccountInfo::new(account.balance, account.nonce, code.hash_slow(), code)
    }
}

impl From<AccountInfo> for AccountBasic {
    fn from(account: AccountInfo) -> Self {
        AccountBasic {
            balance: account.balance,
            nonce: account.nonce,
            code: account.code.unwrap_or_default().original_bytes(),
        }
    }
}

/// An interface to provide chain state to BlockSTM for transaction execution.
/// Staying close to the underlying REVM's Database trait while not leaking
/// its primitives to library users (favoring Alloy at the moment).
/// TODO: Better API for third-pary integration.
pub trait Storage {
    /// Errors when querying data from storage.
    type Error: Debug;

    /// Get basic account information.
    fn basic(&self, address: Address) -> Result<Option<AccountBasic>, Self::Error>;

    /// Get account code by its hash.
    fn code_by_hash(&self, code_hash: B256) -> Result<Bytes, Self::Error>;

    /// Get if the account already has storage (to support EIP-7610).
    fn has_storage(&self, address: Address) -> Result<bool, Self::Error>;

    /// Get storage value of address at index.
    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error>;

    /// Get block hash by block number.
    fn block_hash(&self, number: U256) -> Result<B256, Self::Error>;
}

// We can use any REVM database as storage provider.
// There are some unfortunate back-and-forth conversions between
// account & byte types, which hopefully it is minor enough.
// TODO: More proper testing.
impl<D: DatabaseRef> Storage for D
where
    D::Error: Debug,
{
    type Error = D::Error;

    fn basic(&self, address: Address) -> Result<Option<AccountBasic>, Self::Error> {
        D::basic_ref(self, address).map(|a| a.map(|a| a.into()))
    }

    fn code_by_hash(&self, code_hash: B256) -> Result<Bytes, Self::Error> {
        D::code_by_hash_ref(self, code_hash).map(|b| b.original_bytes())
    }

    fn has_storage(&self, address: Address) -> Result<bool, Self::Error> {
        D::has_storage_ref(self, address)
    }

    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        D::storage_ref(self, address, index)
    }

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        D::block_hash_ref(self, number)
    }
}

// RPC Storage
// TODO: Move to its own module.

// TODO: Support generic network & transport types.
// TODO: Put this behind an RPC flag to not pollute the core
// library with RPC network & transport dependencies, etc.
type RpcProvider = RootProvider<Http<Client>>;

/// Fetch state data via RPC to execute.
#[derive(Debug)]
pub struct RpcStorage {
    provider: RpcProvider,
    block_id: BlockId,
    // TODO: Better async handling.
    runtime: Runtime,
}

impl RpcStorage {
    /// Create a new RPC Storage
    pub fn new(provider: RpcProvider, block_id: BlockId) -> Self {
        RpcStorage {
            provider,
            block_id,
            // TODO: Better error handling.
            runtime: Runtime::new().unwrap(),
        }
    }
}

// TODO: Implement `Storage` instead.
// Going with REVM's Database simply to make it easier
// to try matching sequential & parallel execution.
// In the future we should match block roots anyway.
impl DatabaseRef for RpcStorage {
    type Error = TransportError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.runtime.block_on(async {
            // TODO: Request these concurrently
            let (balance, nonce, code) = tokio::join!(
                self.provider
                    .get_balance(address)
                    .block_id(self.block_id)
                    .into_future(),
                self.provider
                    .get_transaction_count(address)
                    .block_id(self.block_id)
                    .into_future(),
                self.provider
                    .get_code_at(address)
                    .block_id(self.block_id)
                    .into_future()
            );
            // TODO: Should we properly cover the non-existing account case or it can
            // always be a `Some` here?
            let code = Bytecode::new_raw(code?);
            Ok(Some(AccountInfo::new(
                balance?,
                nonce?,
                code.hash_slow(),
                code,
            )))
        })
    }

    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called as the code is already loaded via account");
    }

    fn has_storage_ref(&self, _address: Address) -> Result<bool, Self::Error> {
        // FIXME! Returning `false` that should cover EIP-7610 for the time being.
        Ok(false)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.runtime.block_on(
            self.provider
                .get_storage_at(address, index)
                .block_id(self.block_id)
                .into_future(),
        )
    }

    // TODO: Proper error handling & testing.
    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        self.runtime
            .block_on(
                self.provider
                    .get_block_by_number(BlockNumberOrTag::Number(number.to::<u64>()), false)
                    .into_future(),
            )
            .map(|block| block.unwrap().header.hash.unwrap())
    }
}

// TODO: Reintroduce our own lightweight in-memory storage instead
// of using REVM's InMemoryDB for testing. The former is cleaner
// but its demand is too low to justify the maintenance cost.
// Perhaps we add it once we support multiple underlying executors.
