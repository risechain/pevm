// A storage that fetches state data via RPC for execution.

use std::{collections::HashMap, fmt::Debug, future::IntoFuture, sync::Mutex};

use alloy_primitives::{Address, B256, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::{BlockId, BlockNumberOrTag};
use alloy_transport::TransportError;
use alloy_transport_http::Http;
use reqwest::Client;
use revm::{
    db::PlainAccount,
    primitives::{AccountInfo, Bytecode},
    DatabaseRef,
};
use tokio::runtime::Runtime;

// TODO: Support generic network & transport types.
// TODO: Put this behind an RPC flag to not pollute the core
// library with RPC network & transport dependencies, etc.
type RpcProvider = RootProvider<Http<Client>>;

/// Fetch state data via RPC to execute.
#[derive(Debug)]
pub struct RpcStorage {
    provider: RpcProvider,
    block_id: BlockId,
    // Convenient types for persisting then reconstructing block's state
    // as in-memory storage for benchmarks, etc. Also work well when
    // the storage is re-used, like for comparing sequential & parallel
    // execution on the same block.
    // Using a `Mutex` so we don't (yet) propagate mutability requirements
    // back to our `Storage` trait.
    // Not using `AHashMap` for ease of serialization.
    // TODO: Cache & snapshot block hashes too!
    cache: Mutex<HashMap<Address, PlainAccount>>,
    // TODO: Better async handling.
    runtime: Runtime,
}

impl RpcStorage {
    /// Create a new RPC Storage
    pub fn new(provider: RpcProvider, block_id: BlockId) -> Self {
        RpcStorage {
            provider,
            block_id,
            cache: Mutex::new(HashMap::new()),
            // TODO: Better error handling.
            runtime: Runtime::new().unwrap(),
        }
    }

    /// Get a snapshot of the loaded state
    pub fn get_cache(&self) -> HashMap<Address, PlainAccount> {
        self.cache.lock().unwrap().clone()
    }
}

// TODO: Implement `Storage` instead.
// Going with REVM's Database simply to make it easier
// to try matching sequential & parallel execution.
// In the future we should match block roots anyway.
impl DatabaseRef for RpcStorage {
    type Error = TransportError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account) = self.cache.lock().unwrap().get(&address) {
            return Ok(Some(account.info.clone()));
        }
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
            let info = AccountInfo::new(balance?, nonce?, code.hash_slow(), code);
            self.cache
                .lock()
                .unwrap()
                .insert(address, info.clone().into());
            Ok(Some(info))
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
        if let Some(account) = self.cache.lock().unwrap().get(&address) {
            if let Some(value) = account.storage.get(&index) {
                return Ok(*value);
            }
        }
        let value = self.runtime.block_on(
            self.provider
                .get_storage_at(address, index)
                .block_id(self.block_id)
                .into_future(),
        )?;
        match self.cache.lock().unwrap().entry(address) {
            std::collections::hash_map::Entry::Occupied(mut account) => {
                account.get_mut().storage.insert(index, value);
            }
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(PlainAccount {
                    info: self.basic_ref(address)?.unwrap_or_default(),
                    storage: [(index, value)].into_iter().collect(),
                });
            }
        };
        Ok(value)
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
