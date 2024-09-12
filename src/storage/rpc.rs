// TODO: Put this behind an RPC flag to not pollute the core
// library with RPC network & transport dependencies, etc.

use std::{fmt::Debug, future::IntoFuture, sync::Mutex, time::Duration};

use ahash::AHashMap;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::{
    network::{BlockResponse, HeaderResponse},
    Network, Provider, RootProvider,
};
use alloy_rpc_types::{BlockId, BlockNumberOrTag};
use alloy_transport::TransportError;
use alloy_transport_http::Http;
use reqwest::Client;
use revm::{
    precompile::{PrecompileSpecId, Precompiles},
    primitives::{Bytecode, SpecId},
};
use tokio::runtime::Runtime;

use crate::{AccountBasic, EvmAccount, Storage};

use super::{BlockHashes, Bytecodes, ChainState, EvmCode};

type RpcProvider<N> = RootProvider<Http<Client>, N>;

/// A storage that fetches state data via RPC for execution.
#[derive(Debug)]
pub struct RpcStorage<N> {
    provider: RpcProvider<N>,
    block_id: BlockId,
    precompiles: &'static Precompiles,
    // Convenient types for persisting then reconstructing block's state
    // as in-memory storage for benchmarks & testing. Also work well when
    // the storage is re-used, like for comparing sequential & parallel
    // execution on the same block.
    // Using a [Mutex] so we don't propagate mutability requirements back
    // to our [Storage] trait and meet [Send]/[Sync] requirements for Pevm.
    cache_accounts: Mutex<ChainState>,
    cache_bytecodes: Mutex<Bytecodes>,
    cache_block_hashes: Mutex<BlockHashes>,
    // TODO: Better async handling.
    runtime: Runtime,
}

impl<N> RpcStorage<N> {
    /// Create a new RPC Storage
    pub fn new(provider: RpcProvider<N>, spec_id: SpecId, block_id: BlockId) -> Self {
        RpcStorage {
            provider,
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec_id)),
            block_id,
            cache_accounts: Mutex::default(),
            cache_bytecodes: Mutex::default(),
            cache_block_hashes: Mutex::default(),
            // TODO: Better error handling.
            runtime: Runtime::new().unwrap(),
        }
    }

    /// Send a request and retry many times if needed.
    /// This util is made to avoid error 429 Too Many Requests
    /// https://en.wikipedia.org/wiki/Exponential_backoff
    fn fetch<T, E, R: IntoFuture<Output = Result<T, E>>>(
        &self,
        request: impl Fn() -> R,
    ) -> Result<T, E> {
        const RETRY_LIMIT: usize = 8;
        const INITIAL_DELAY_MILLIS: u64 = 125;

        self.runtime.block_on(async {
            let mut lives = RETRY_LIMIT;
            let mut delay = Duration::from_millis(INITIAL_DELAY_MILLIS);

            loop {
                let result = request().await;
                if lives > 0 && result.is_err() {
                    tokio::time::sleep(delay).await;
                    lives -= 1;
                    delay *= 2;
                } else {
                    return result;
                }
            }
        })
    }

    /// Get a snapshot of accounts
    pub fn get_cache_accounts(&self) -> ChainState {
        self.cache_accounts.lock().unwrap().clone()
    }

    /// Get a snapshot of bytecodes
    pub fn get_cache_bytecodes(&self) -> Bytecodes {
        self.cache_bytecodes.lock().unwrap().clone()
    }

    /// Get a snapshot of block hashes
    pub fn get_cache_block_hashes(&self) -> BlockHashes {
        self.cache_block_hashes.lock().unwrap().clone()
    }
}

impl<N: Network> Storage for RpcStorage<N> {
    type Error = TransportError;

    fn basic(&self, address: &Address) -> Result<Option<AccountBasic>, Self::Error> {
        if let Some(account) = self.cache_accounts.lock().unwrap().get(address) {
            return Ok(Some(AccountBasic {
                balance: account.balance,
                nonce: account.nonce,
            }));
        }
        let nonce = self.fetch(|| {
            self.provider
                .get_transaction_count(*address)
                .block_id(self.block_id)
        })?;
        let balance = self.fetch(|| self.provider.get_balance(*address).block_id(self.block_id))?;
        let code = self.fetch(|| self.provider.get_code_at(*address).block_id(self.block_id))?;

        // We need to distinguish new non-precompile accounts for gas calculation
        // in early hard-forks (creating new accounts cost extra gas, etc.).
        if !self
            .precompiles
            .addresses()
            .any(|precompile_address| precompile_address == address)
            && balance.is_zero()
            && nonce == 0
            && code.is_empty()
        {
            return Ok(None);
        }
        let code = Bytecode::new_raw(code);
        let code_hash = if code.is_empty() {
            None
        } else {
            let code_hash = code.hash_slow();
            self.cache_bytecodes
                .lock()
                .unwrap()
                .insert(code_hash, code.into());
            Some(code_hash)
        };
        self.cache_accounts.lock().unwrap().insert(
            *address,
            EvmAccount {
                balance,
                nonce,
                code_hash,
                code: None,
                storage: AHashMap::default(),
            },
        );
        Ok(Some(AccountBasic { balance, nonce }))
    }

    fn code_hash(&self, address: &Address) -> Result<Option<B256>, Self::Error> {
        self.basic(address)?;
        Ok(self
            .cache_accounts
            .lock()
            .unwrap()
            .get(address)
            .and_then(|account| account.code_hash))
    }

    fn code_by_hash(&self, code_hash: &B256) -> Result<Option<EvmCode>, Self::Error> {
        Ok(self.cache_bytecodes.lock().unwrap().get(code_hash).cloned())
    }

    fn has_storage(&self, address: &Address) -> Result<bool, Self::Error> {
        let proof = self.fetch(|| {
            self.provider
                // [get_account] is simpler but it yields deserialization
                // error on an empty account.
                .get_proof(*address, Vec::new())
                .block_id(self.block_id)
        })?;
        Ok(proof.storage_hash != alloy_consensus::EMPTY_ROOT_HASH)
    }

    fn storage(&self, address: &Address, index: &U256) -> Result<U256, Self::Error> {
        if let Some(account) = self.cache_accounts.lock().unwrap().get(address) {
            if let Some(value) = account.storage.get(index) {
                return Ok(*value);
            }
        }
        let value = self.fetch(|| {
            self.provider
                .get_storage_at(*address, *index)
                .block_id(self.block_id)
        })?;
        // We only cache if the pre-state account is non-empty. Else this
        // could be a false alarm that results in the default 0. Caching
        // that would make this account non-empty and may fail a tx that
        // deploys a contract here (EIP-7610).
        self.basic(address)?;
        if let Some(account) = self.cache_accounts.lock().unwrap().get_mut(address) {
            account.storage.insert(*index, value);
        }

        Ok(value)
    }

    fn block_hash(&self, number: &u64) -> Result<B256, Self::Error> {
        if let Some(&block_hash) = self.cache_block_hashes.lock().unwrap().get(number) {
            return Ok(block_hash);
        }

        let block_hash = self
            .fetch(|| {
                self.provider
                    .get_block_by_number(BlockNumberOrTag::Number(*number), false)
            })
            .map(|block| block.unwrap().header().hash())?;

        self.cache_block_hashes
            .lock()
            .unwrap()
            .insert(*number, block_hash);

        Ok(block_hash)
    }
}
