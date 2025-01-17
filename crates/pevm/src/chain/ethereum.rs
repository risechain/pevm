//! Ethereum

use alloy_consensus::{ReceiptEnvelope, Transaction, TxEnvelope, TxType};
use alloy_primitives::{Address, B256, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types_eth::{BlockTransactions, Header};
use hashbrown::HashMap;
use revm::{
    primitives::{AuthorizationList, BlockEnv, SpecId, TxEnv},
    Handler,
};

use super::{CalculateReceiptRootError, PevmChain, RewardPolicy};
use crate::{
    hash_determinisitic, mv_memory::MvMemory, BuildIdentityHasher, MemoryLocation,
    PevmTxExecutionResult, TxIdx,
};

/// Implementation of [`PevmChain`] for Ethereum
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PevmEthereum {
    id: u64,
}

impl PevmEthereum {
    /// Ethereum Mainnet
    pub const fn mainnet() -> Self {
        Self { id: 1 }
    }

    // TODO: support Ethereum Sepolia and other testnets
}

/// Represents errors that can occur when parsing transactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthereumTransactionParsingError {
    /// [`tx.gas_price`] is none.
    MissingGasPrice,
}

fn get_ethereum_gas_price(tx: &TxEnvelope) -> Result<U256, EthereumTransactionParsingError> {
    match tx.tx_type() {
        TxType::Legacy | TxType::Eip2930 => tx
            .gas_price()
            .map(U256::from)
            .ok_or(EthereumTransactionParsingError::MissingGasPrice),
        TxType::Eip1559 | TxType::Eip4844 | TxType::Eip7702 => Ok(U256::from(tx.max_fee_per_gas())),
    }
}

impl PevmChain for PevmEthereum {
    type Transaction = alloy_rpc_types_eth::Transaction;
    type Envelope = TxEnvelope;
    type BlockSpecError = ();
    type TransactionParsingError = EthereumTransactionParsingError;

    fn id(&self) -> u64 {
        self.id
    }

    fn mock_tx(&self, envelope: Self::Envelope, from: Address) -> Self::Transaction {
        Self::mock_rpc_tx(envelope, from)
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from these references:
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
    // TODO: Better error handling & properly test this.
    // TODO: Only Ethereum Mainnet is supported at the moment.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        Ok(if header.timestamp >= 1710338135 {
            SpecId::CANCUN
        } else if header.timestamp >= 1681338455 {
            SpecId::SHANGHAI
        }
        // Checking for total difficulty is more precise but many RPC providers stopped returning it...
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
        })
    }

    /// Get the REVM tx envs of an Alloy block.
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/revm/env.rs#L234-L339
    // https://github.com/paradigmxyz/reth/blob/280aaaedc4699c14a5b6e88f25d929fe22642fa3/crates/primitives/src/alloy_compat.rs#L112-L233
    // TODO: Properly test this.
    fn get_tx_env(&self, tx: &Self::Transaction) -> Result<TxEnv, EthereumTransactionParsingError> {
        Ok(TxEnv {
            caller: tx.from,
            gas_limit: tx.gas_limit(),
            gas_price: get_ethereum_gas_price(&tx.inner)?,
            gas_priority_fee: tx.max_priority_fee_per_gas().map(U256::from),
            transact_to: tx.kind(),
            value: tx.value(),
            data: tx.input().clone(),
            nonce: Some(tx.nonce()),
            chain_id: tx.chain_id(),
            access_list: tx.access_list().cloned().unwrap_or_default().to_vec(),
            blob_hashes: tx.blob_versioned_hashes().unwrap_or_default().to_vec(),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas().map(U256::from),
            authorization_list: tx
                .authorization_list()
                .map(|auths| AuthorizationList::Signed(auths.to_vec())),
            #[cfg(feature = "optimism")]
            optimism: revm::primitives::OptimismFields::default(),
        })
    }

    fn build_mv_memory(&self, block_env: &BlockEnv, txs: &[TxEnv]) -> MvMemory {
        let block_size = txs.len();
        let beneficiary_location_hash =
            hash_determinisitic(MemoryLocation::Basic(block_env.coinbase));

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        estimated_locations.insert(
            beneficiary_location_hash,
            (0..block_size).collect::<Vec<TxIdx>>(),
        );

        MvMemory::new(block_size, estimated_locations, [block_env.coinbase])
    }

    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB> {
        Handler::mainnet_with_spec(spec_id, with_reward_beneficiary)
    }

    fn get_reward_policy(&self) -> RewardPolicy {
        RewardPolicy::Ethereum
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
