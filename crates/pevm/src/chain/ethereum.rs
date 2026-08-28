//! Ethereum

use alloy_consensus::{ReceiptEnvelope, Transaction, TxEnvelope, TxType};
use alloy_primitives::{Address, B256, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types_eth::{BlockTransactions, Header};
use hashbrown::HashMap;
use revm::{
    Context, Database, MainBuilder, MainContext, MainnetEvm,
    context::{
        BlockEnv, CfgEnv, TxEnv,
        result::{HaltReason, InvalidTransaction},
    },
    context_interface::either::Either,
    handler::MainnetContext,
    primitives::{
        eip4844::{MAX_BLOB_NUMBER_PER_BLOCK_CANCUN, MAX_BLOB_NUMBER_PER_BLOCK_PRAGUE},
        hardfork::SpecId,
    },
};
use smallvec::SmallVec;

use super::{CalculateReceiptRootError, PevmChain};
use crate::{
    BuildIdentityHasher, MemoryLocation, MemoryLocationHash, PevmTxExecutionResult, TxIdx,
    hash_deterministic, mv_memory::MvMemory,
};

const SEPOLIA_CHAIN_ID: u64 = 11_155_111;
const SEPOLIA_PARIS_BLOCK: u64 = 1_450_409;
const SEPOLIA_SHANGHAI_TIMESTAMP: u64 = 1_677_557_088;
const SEPOLIA_CANCUN_TIMESTAMP: u64 = 1_706_655_072;
const SEPOLIA_PRAGUE_TIMESTAMP: u64 = 1_741_159_776;
const SEPOLIA_OSAKA_TIMESTAMP: u64 = 1_760_427_360;

/// Implementation of [`PevmChain`] for Ethereum
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PevmEthereum {
    network: EthereumNetwork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EthereumNetwork {
    Mainnet,
    Sepolia,
}

impl PevmEthereum {
    /// Ethereum Mainnet
    pub const fn mainnet() -> Self {
        Self {
            network: EthereumNetwork::Mainnet,
        }
    }

    /// Ethereum Sepolia
    pub const fn sepolia() -> Self {
        Self {
            network: EthereumNetwork::Sepolia,
        }
    }

    fn mainnet_block_spec(header: &Header) -> SpecId {
        if header.timestamp >= 1710338135 {
            SpecId::CANCUN
        } else if header.timestamp >= 1681338455 {
            SpecId::SHANGHAI
        }
        // Checking for total difficulty is more precise but many RPC providers stopped returning
        // it.
        else if header.number >= 15537394 {
            SpecId::MERGE
        } else if header.number >= 12965000 {
            SpecId::LONDON
        } else if header.number >= 12244000 {
            SpecId::BERLIN
        } else if header.number >= 9069000 {
            SpecId::ISTANBUL
        } else if header.number >= 7280000 {
            SpecId::PETERSBURG
        } else if header.number >= 4370000 {
            SpecId::BYZANTIUM
        } else if header.number >= 2675000 {
            SpecId::SPURIOUS_DRAGON
        } else if header.number >= 2463000 {
            SpecId::TANGERINE
        } else if header.number >= 1150000 {
            SpecId::HOMESTEAD
        } else {
            SpecId::FRONTIER
        }
    }

    fn sepolia_block_spec(header: &Header) -> SpecId {
        // Sepolia's activation points are defined by alloy-hardforks:
        // https://github.com/alloy-rs/hardforks/blob/a8af395408a4850fab5e0ca708cf495477c61bcc/crates/hardforks/src/ethereum/sepolia.rs
        if header.timestamp >= SEPOLIA_OSAKA_TIMESTAMP {
            SpecId::OSAKA
        } else if header.timestamp >= SEPOLIA_PRAGUE_TIMESTAMP {
            SpecId::PRAGUE
        } else if header.timestamp >= SEPOLIA_CANCUN_TIMESTAMP {
            SpecId::CANCUN
        } else if header.timestamp >= SEPOLIA_SHANGHAI_TIMESTAMP {
            SpecId::SHANGHAI
        } else if header.number >= SEPOLIA_PARIS_BLOCK {
            // Checking for total difficulty is more precise, but it is no longer reliably
            // returned by RPC providers.
            SpecId::MERGE
        } else {
            // All pre-merge hardforks through London were active at Sepolia genesis.
            SpecId::LONDON
        }
    }
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EthereumTransactionParsingError {
    /// [`tx.gas_price`] is none.
    #[error("Missing gas price")]
    MissingGasPrice,
}

fn get_ethereum_gas_price(tx: &TxEnvelope) -> Result<u128, EthereumTransactionParsingError> {
    match tx.tx_type() {
        TxType::Legacy | TxType::Eip2930 => tx
            .gas_price()
            .ok_or(EthereumTransactionParsingError::MissingGasPrice),
        TxType::Eip1559 | TxType::Eip4844 | TxType::Eip7702 => Ok(tx.max_fee_per_gas()),
    }
}

impl PevmChain for PevmEthereum {
    type Network = alloy_provider::network::Ethereum;
    type Transaction = alloy_rpc_types_eth::Transaction;
    type Envelope = TxEnvelope;
    type Evm<DB: Database> = MainnetEvm<MainnetContext<DB>>;
    type EvmSpecId = SpecId;
    type EvmTx = TxEnv;
    type EvmHaltReason = HaltReason;
    type EvmErrorType = InvalidTransaction;
    type BlockSpecError = std::convert::Infallible;
    type TransactionParsingError = EthereumTransactionParsingError;

    fn id(&self) -> u64 {
        match self.network {
            EthereumNetwork::Mainnet => 1,
            EthereumNetwork::Sepolia => SEPOLIA_CHAIN_ID,
        }
    }

    fn mock_tx(&self, envelope: Self::Envelope, from: Address) -> Self::Transaction {
        Self::mock_rpc_tx(envelope, from)
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from the canonical chain specs.
    // TODO: Better error handling & properly test this.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        Ok(match self.network {
            EthereumNetwork::Mainnet => Self::mainnet_block_spec(header),
            EthereumNetwork::Sepolia => Self::sepolia_block_spec(header),
        })
    }

    fn build_evm<DB: Database>(
        &self,
        spec_id: Self::EvmSpecId,
        block_env: BlockEnv,
        db: DB,
    ) -> Self::Evm<DB> {
        let mut cfg = CfgEnv::new_with_spec(spec_id).with_chain_id(self.id());
        if spec_id >= SpecId::PRAGUE {
            cfg = cfg.with_max_blobs_per_tx(MAX_BLOB_NUMBER_PER_BLOCK_PRAGUE);
        } else if spec_id >= SpecId::CANCUN {
            cfg = cfg.with_max_blobs_per_tx(MAX_BLOB_NUMBER_PER_BLOCK_CANCUN);
        }
        Context::mainnet()
            .with_cfg(cfg)
            .with_block(block_env)
            .with_db(db)
            .build_mainnet()
    }

    /// Get the REVM tx envs of an Alloy block.
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L234-L339
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/alloy_compat.rs#L112-L233
    // TODO: Properly test this.
    fn get_tx_env(&self, tx: &Self::Transaction) -> Result<TxEnv, EthereumTransactionParsingError> {
        Ok(TxEnv {
            tx_type: tx.inner.tx_type().into(),
            caller: tx.inner.signer(),
            gas_limit: tx.gas_limit(),
            gas_price: get_ethereum_gas_price(&tx.inner)?,
            gas_priority_fee: tx.max_priority_fee_per_gas(),
            kind: tx.kind(),
            value: tx.value(),
            data: tx.input().clone(),
            nonce: tx.nonce(),
            chain_id: tx.chain_id(),
            access_list: tx.access_list().cloned().unwrap_or_default(),
            blob_hashes: tx.blob_versioned_hashes().unwrap_or_default().to_vec(),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas().unwrap_or_default(),
            authorization_list: tx
                .authorization_list()
                .map(|auths| auths.iter().cloned().map(Either::Left).collect())
                .unwrap_or_default(),
        })
    }

    fn tx_env<'a>(&self, tx: &'a TxEnv) -> &'a TxEnv {
        tx
    }

    fn build_mv_memory(&self, block_env: &BlockEnv, txs: &[TxEnv]) -> MvMemory {
        let block_size = txs.len();
        let beneficiary_location_hash =
            hash_deterministic(MemoryLocation::Basic(block_env.beneficiary));

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        estimated_locations.insert(
            beneficiary_location_hash,
            (0..block_size).collect::<Vec<TxIdx>>(),
        );

        MvMemory::new(block_size, estimated_locations, [block_env.beneficiary])
    }

    fn get_rewards(
        &self,
        beneficiary_location_hash: u64,
        gas_used: U256,
        gas_price: U256,
        _: u64,
        _: &Self::EvmTx,
    ) -> SmallVec<[(MemoryLocationHash, U256); 1]> {
        smallvec::smallvec![(
            beneficiary_location_hash,
            gas_price.saturating_mul(gas_used)
        )]
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    fn calculate_receipt_root(
        &self,
        spec_id: SpecId,
        txs: &BlockTransactions<Self::Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> Result<B256, CalculateReceiptRootError> {
        if spec_id < SpecId::BYZANTIUM {
            // We can only calculate the receipts root from Byzantium.
            // Before EIP-658 (https://eips.ethereum.org/EIPS/eip-658), the
            // receipt root is calculated with the post transaction state root,
            // which we don't have here.

            // TODO: Allow to calculate the receipt root by providing the post
            // transaction state root.
            return Err(CalculateReceiptRootError::Unsupported);
        }

        let mut trie_entries = txs
            .txns()
            .map(|tx| tx.inner.tx_type())
            .zip(tx_results)
            .map(|(tx_type, tx_result)| {
                let receipt = tx_result.receipt.clone().with_bloom();
                match tx_type {
                    TxType::Legacy => ReceiptEnvelope::Legacy(receipt),
                    TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt),
                    TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt),
                    TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt),
                    TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt),
                }
            })
            .enumerate()
            .map(|(index, receipt)| (alloy_rlp::encode_fixed_size(&index), receipt.encoded_2718()))
            .collect::<Vec<_>>();
        trie_entries.sort();

        let mut hash_builder = alloy_trie::HashBuilder::default();
        for (k, v) in trie_entries {
            hash_builder.add_leaf(alloy_trie::Nibbles::unpack(&k), &v);
        }
        Ok(hash_builder.root())
    }

    fn is_eip_1559_enabled(&self, spec_id: SpecId) -> bool {
        spec_id >= SpecId::LONDON
    }

    fn is_eip_161_enabled(&self, spec_id: SpecId) -> bool {
        spec_id >= SpecId::SPURIOUS_DRAGON
    }
}

#[cfg(test)]
mod tests {
    use alloy_consensus::Header as ConsensusHeader;

    use super::*;

    fn header(number: u64, timestamp: u64) -> Header {
        Header {
            inner: ConsensusHeader {
                number,
                timestamp,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn sepolia_has_the_expected_chain_id() {
        assert_eq!(PevmEthereum::sepolia().id(), SEPOLIA_CHAIN_ID);
    }

    #[test]
    fn sepolia_uses_london_before_the_merge() {
        let chain = PevmEthereum::sepolia();
        assert_eq!(chain.get_block_spec(&header(0, 0)), Ok(SpecId::LONDON));
        assert_eq!(
            chain.get_block_spec(&header(SEPOLIA_PARIS_BLOCK - 1, 1_633_267_480)),
            Ok(SpecId::LONDON)
        );
    }

    #[test]
    fn sepolia_activates_each_supported_hardfork_at_its_boundary() {
        let chain = PevmEthereum::sepolia();
        let cases = [
            (SEPOLIA_PARIS_BLOCK, 0, SpecId::MERGE),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_SHANGHAI_TIMESTAMP - 1,
                SpecId::MERGE,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_SHANGHAI_TIMESTAMP,
                SpecId::SHANGHAI,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_CANCUN_TIMESTAMP - 1,
                SpecId::SHANGHAI,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_CANCUN_TIMESTAMP,
                SpecId::CANCUN,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_PRAGUE_TIMESTAMP - 1,
                SpecId::CANCUN,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_PRAGUE_TIMESTAMP,
                SpecId::PRAGUE,
            ),
            (
                SEPOLIA_PARIS_BLOCK,
                SEPOLIA_OSAKA_TIMESTAMP - 1,
                SpecId::PRAGUE,
            ),
            (SEPOLIA_PARIS_BLOCK, SEPOLIA_OSAKA_TIMESTAMP, SpecId::OSAKA),
        ];

        for (number, timestamp, expected) in cases {
            assert_eq!(
                chain.get_block_spec(&header(number, timestamp)),
                Ok(expected),
                "unexpected spec at block {number}, timestamp {timestamp}"
            );
        }
    }
}
