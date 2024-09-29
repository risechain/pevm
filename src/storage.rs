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
    pub bytecode: Bytes,
    /// Original bytes length.
    pub original_len: usize,
    /// Jump table.
    pub jump_table: Arc<BitVec<u8>>,
}

/// EIP7702 delegated code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eip7702Code {
    /// Address of the EOA which will inherit the bytecode.
    pub delegated_address: Address,
    /// Version of the bytecode.
    pub version: u8,
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
            Bytecode::Eof(_) => unimplemented!("TODO: Support EOF"),
            Bytecode::Eip7702(code) => EvmCode::Eip7702(Eip7702Code {
                delegated_address: code.delegated_address,
                version: code.version,
            }),
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
    use std::str::FromStr;

    use alloy_primitives::bytes;

    use super::*;

    const BYTECODE: alloy_primitives::Bytes = bytes!("608060405234801561001057600080fd5b506004361061002b5760003560e01c8063920a769114610030575b600080fd5b61004361003e366004610374565b610055565b60405190815260200160405180910390f35b600061006082610067565b5192915050565b60606101e0565b818153600101919050565b600082840393505b838110156100a25782810151828201511860001a1590930292600101610081565b9392505050565b825b602082106100d75782516100c0601f8361006e565b5260209290920191601f19909101906021016100ab565b81156100a25782516100ec600184038361006e565b520160010192915050565b60006001830392505b61010782106101385761012a8360ff1661012560fd6101258760081c60e0018961006e565b61006e565b935061010682039150610100565b600782106101655761015e8360ff16610125600785036101258760081c60e0018961006e565b90506100a2565b61017e8360ff166101258560081c8560051b018761006e565b949350505050565b80516101d890838303906101bc90600081901a600182901a60081b1760029190911a60101b17639e3779b90260131c611fff1690565b8060021b6040510182815160e01c1860e01b8151188152505050565b600101919050565b5060405161800038823961800081016020830180600d8551820103826002015b81811015610313576000805b50508051604051600082901a600183901a60081b1760029290921a60101b91909117639e3779b9810260111c617ffc16909101805160e081811c878603811890911b9091189091528401908183039084841061026857506102a3565b600184019350611fff821161029d578251600081901a600182901a60081b1760029190911a60101b17810361029d57506102a3565b5061020c565b8383106102b1575050610313565b600183039250858311156102cf576102cc87878886036100a9565b96505b6102e3600985016003850160038501610079565b91506102f08782846100f7565b9650506103088461030386848601610186565b610186565b915050809350610200565b5050617fe061032884848589518601036100a9565b03925050506020820180820383525b81811161034e57617fe08101518152602001610337565b5060008152602001604052919050565b634e487b7160e01b600052604160045260246000fd5b60006020828403121561038657600080fd5b813567ffffffffffffffff8082111561039e57600080fd5b818401915084601f8301126103b257600080fd5b8135818111156103c4576103c461035e565b604051601f8201601f19908116603f011681019083821181831017156103ec576103ec61035e565b8160405282815287602084870101111561040557600080fd5b82602086016020830137600092810160200192909252509594505050505056fea264697066735822122000646b2953fc4a6f501bd0456ac52203089443937719e16b3190b7979c39511264736f6c63430008190033");

    #[test]
    fn test_evmcode_from_revm_bytecode_eip7702() {
        let addr = Address::new([0x01; 20]);

        // New from address.
        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new(addr));
        let evmcode = EvmCode::from(bytecode);
        assert!(
            matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, .. })
                if delegated_address == addr
            )
        );

        // New from raw.
        let byte_str = format!("ef0100{}", addr.to_string().trim_start_matches("0x"));
        let raw = Bytes::from_str(&byte_str).unwrap();
        let bytecode = Bytecode::Eip7702(Eip7702Bytecode::new_raw(raw).unwrap());
        let evmcode = EvmCode::from(bytecode);
        assert!(
            matches!(evmcode, EvmCode::Eip7702(Eip7702Code { delegated_address, version })
                if delegated_address == addr && version == 0
            )
        );
    }

    #[test]
    fn test_evmcode_from_revm_bytecode_legacy_raw() {
        let contract_bytecode = Bytecode::new_legacy(BYTECODE);
        let analyzed = to_analysed(contract_bytecode.clone());
        // Create EvmCode from raw Bytecode.
        let evmcode = EvmCode::from(contract_bytecode);

        if let Bytecode::LegacyAnalyzed(legacy_analyzed) = analyzed {
            let raw_jump = legacy_analyzed.jump_table().0.clone();
            assert!(
                matches!(evmcode, EvmCode::Legacy(LegacyCode { bytecode, original_len, jump_table })
                    if bytecode == *legacy_analyzed.bytecode() && original_len == legacy_analyzed.original_len() && jump_table == raw_jump
                )
            );
        } else {
            panic!("Expected LegacyAnalyzed Bytecode")
        }
    }

    #[test]
    fn test_evmcode_from_revm_bytecode_legacy_analyzed() {
        let contract_bytecode = Bytecode::new_legacy(BYTECODE);
        let analyzed = to_analysed(contract_bytecode.clone());
        // Create EvmCode from analyzed bytecode.
        let evmcode = EvmCode::from(analyzed.clone());

        if let Bytecode::LegacyAnalyzed(legacy_analyzed) = analyzed {
            let raw_jump = legacy_analyzed.jump_table().0.clone();
            assert!(
                matches!(evmcode, EvmCode::Legacy(LegacyCode { bytecode, original_len, jump_table })
                    if bytecode == *legacy_analyzed.bytecode() && original_len == legacy_analyzed.original_len() && jump_table == raw_jump
                )
            );
        } else {
            panic!("Expected LegacyAnalyzed Bytecode")
        }
    }
}
