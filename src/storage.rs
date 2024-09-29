use std::{collections::HashMap, fmt::Display, sync::Arc};

use ahash::AHashMap;
use alloy_primitives::{Address, Bytes, B256, U256};
use bitvec::vec::BitVec;
use revm::{
    interpreter::analysis::to_analysed,
    primitives::{
        Account, AccountInfo, Bytecode, Eip7702Bytecode, JumpTable, EIP7702_MAGIC_BYTES,
        KECCAK_EMPTY,
    },
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

/// Analyzed legacy code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyCode {
    /// Bytecode with 32 zero bytes padding.
    // TODO: Store unpadded bytecode and pad on revm conversion.
    bytecode: Bytes,
    /// Original bytes length.
    original_len: usize,
    /// Jump table.
    jump_table: Arc<BitVec<u8>>,
}

/// EIP7702 delegated code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eip7702Code {
    /// Address of the EOA which will inherit the bytecode.
    delegated_address: Address,
    /// Version of the bytecode.
    version: u8,
}

/// EVM Code, currently mapping to REVM's [ByteCode].
// TODO: Support EOF
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvmCode {
    /// Maps both analyzed and non-analyzed REVM legacy bytecode.
    Legacy(LegacyCode),
    /// Maps delegated EIP7702 bytecode.
    Eip7702(Eip7702Code),
}

impl From<EvmCode> for Bytecode {
    fn from(code: EvmCode) -> Self {
        // TODO: Better error handling.
        // A common trap would be converting a default [EvmCode] into
        // a [Bytecode]. On failure we should fallback to legacy and
        // analyse again.
        match code {
            EvmCode::Eip7702(code) => {
                let mut raw = EIP7702_MAGIC_BYTES.to_vec();
                raw.push(code.version);
                raw.extend(&code.delegated_address);
                Bytecode::Eip7702(Eip7702Bytecode {
                    delegated_address: code.delegated_address,
                    version: code.version,
                    raw: raw.into(),
                })
            }
            EvmCode::Legacy(code) => unsafe {
                Bytecode::new_analyzed(code.bytecode, code.original_len, JumpTable(code.jump_table))
            },
        }
    }
}

impl From<Bytecode> for EvmCode {
    fn from(code: Bytecode) -> Self {
        match code {
            // This arm will recursively fallback to LegacyAnalyzed.
            Bytecode::LegacyRaw(_) => to_analysed(code).into(),
            Bytecode::LegacyAnalyzed(code) => EvmCode::Legacy(LegacyCode {
                bytecode: code.bytecode,
                original_len: code.original_len,
                jump_table: code.jump_table.0,
            }),
            Bytecode::Eip7702(code) => EvmCode::Eip7702(Eip7702Code {
                delegated_address: code.delegated_address,
                version: code.version,
            }),
            Bytecode::Eof(_) => unimplemented!("TODO: Support EOF"),
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

/// A Storage wrapper that implements REVM's [DatabaseRef] for ease of
/// integration.
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

#[cfg(test)]
mod tests {
    use alloy_primitives::bytes;
    use revm::primitives::eip7702::EIP7702_VERSION;

    use super::*;

    #[test]
    fn evmcode_from_revm_bytecode_legacy() {
        let contract_bytecode = Bytecode::new_legacy(BYTECODE);
        let analyzed = to_analysed(contract_bytecode.clone());
        // Create EvmCode from analyzed bytecode.
        let evmcode = EvmCode::from(analyzed.clone());
        assert!(eq_bytecodes(&analyzed, &evmcode));
        // Create EvmCode from raw Bytecode.
        let evmcode = EvmCode::from(contract_bytecode);
        assert!(eq_bytecodes(&analyzed, &evmcode));
    }

    #[test]
    fn evmcode_from_revm_bytecode_eip7702() {
        let addr = Address::new([0x01; 20]);

        // New from address.
        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new(addr));
        let evmcode = EvmCode::from(bytecode.clone());
        assert!(
            matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, version })
                if delegated_address == addr && version == EIP7702_VERSION
            )
        );

        // New from raw.
        let mut bytes = EIP7702_MAGIC_BYTES.to_vec();
        bytes.push(EIP7702_VERSION);
        bytes.extend(addr);
        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new_raw(bytes.into()).unwrap());
        let evmcode = EvmCode::from(bytecode);
        assert!(
            matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, version })
                if delegated_address == addr && version == EIP7702_VERSION
            )
        );
    }

    fn eq_bytecodes(revm_code: &Bytecode, pevm_code: &EvmCode) -> bool {
        match (revm_code, pevm_code) {
            (Bytecode::LegacyAnalyzed(revm), EvmCode::Legacy(pevm)) => {
                let raw_jump = revm.jump_table().0.clone();
                revm.bytecode == pevm.bytecode
                    && revm.original_len == pevm.original_len
                    && raw_jump == pevm.jump_table
            }
            _ => false,
        }
    }

    // Bytecode from Storage.sol.
    const BYTECODE: alloy_primitives::Bytes = bytes!("6080604052348015600e575f80fd5b506101438061001c5f395ff3fe608060405234801561000f575f80fd5b5060043610610034575f3560e01c80632e64cec1146100385780636057361d14610056575b5f80fd5b610040610072565b60405161004d919061009b565b60405180910390f35b610070600480360381019061006b91906100e2565b61007a565b005b5f8054905090565b805f8190555050565b5f819050919050565b61009581610083565b82525050565b5f6020820190506100ae5f83018461008c565b92915050565b5f80fd5b6100c181610083565b81146100cb575f80fd5b50565b5f813590506100dc816100b8565b92915050565b5f602082840312156100f7576100f66100b4565b5b5f610104848285016100ce565b9150509291505056fea26469706673582212209a0dd35336aff1eb3eeb11db76aa60a1427a12c1b92f945ea8c8d1dfa337cf2264736f6c634300081a0033");
}
