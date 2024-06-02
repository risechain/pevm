use alloy_rpc_types::{calc_excess_blob_gas, BlockTransactions, Header};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, SpecId, TransactTo, TxEnv, U256};

/// Get the REVM spec id of an Alloy block.
// Currently hardcoding Ethereum hardforks from these reference:
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
// https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
// TODO: Better error handling & properly test this.
pub(crate) fn get_block_spec(header: &Header) -> Option<SpecId> {
    Some(if header.timestamp >= 1710338135 {
        SpecId::CANCUN
    } else if header.timestamp >= 1681338455 {
        SpecId::SHANGHAI
    } else if header.total_difficulty? >= U256::from(58_750_000_000_000_000_000_000_u128) {
        SpecId::MERGE
    } else if header.number? >= 12965000 {
        SpecId::LONDON
    } else if header.number? >= 12244000 {
        SpecId::BERLIN
    } else if header.number? >= 9069000 {
        SpecId::ISTANBUL
    } else if header.number? >= 7280000 {
        SpecId::PETERSBURG
    } else if header.number? >= 4370000 {
        SpecId::BYZANTIUM
    } else if header.number? >= 2675000 {
        SpecId::SPURIOUS_DRAGON
    } else if header.number? >= 2463000 {
        SpecId::TANGERINE
    } else if header.number? >= 1150000 {
        SpecId::HOMESTEAD
    } else {
        SpecId::FRONTIER
    })
}

/// Get the REVM block env of an Alloy block.
// TODO: Better error handling & properly test this.
pub(crate) fn get_block_env(header: &Header, parent: Option<&Header>) -> Option<BlockEnv> {
    Some(BlockEnv {
        number: U256::from(header.number?),
        coinbase: header.miner,
        timestamp: U256::from(header.timestamp),
        gas_limit: U256::from(header.gas_limit),
        basefee: header.base_fee_per_gas.map(U256::from).unwrap_or_default(),
        difficulty: header.difficulty,
        prevrandao: header.mix_hash,
        blob_excess_gas_and_price: if let Some(current_excess_blob_gas) = header.excess_blob_gas {
            Some(BlobExcessGasAndPrice::new(
                current_excess_blob_gas.try_into().ok()?,
            ))
        } else if let (Some(parent_blob_gas_used), Some(parent_excess_blob_gas)) = (
            parent.and_then(|p| p.blob_gas_used),
            parent.and_then(|p| p.excess_blob_gas),
        ) {
            Some(BlobExcessGasAndPrice::new(
                calc_excess_blob_gas(parent_blob_gas_used, parent_excess_blob_gas)
                    .try_into()
                    .ok()?,
            ))
        } else {
            None
        },
    })
}

/// Get the REVM tx envs of an Alloy block.
// TODO: Better error handling & properly test this.
pub(crate) fn get_tx_envs(transactions: &BlockTransactions) -> Option<Vec<TxEnv>> {
    match transactions {
        BlockTransactions::Full(txs) => {
            let mut tx_envs = Vec::new();
            for tx in txs {
                tx_envs.push(TxEnv {
                    caller: tx.from,
                    gas_limit: tx.gas.try_into().ok()?,
                    gas_price: U256::from(tx.gas_price?),
                    transact_to: match tx.to {
                        Some(address) => TransactTo::Call(address),
                        None => TransactTo::Create,
                    },
                    value: tx.value,
                    data: tx.input.clone(),
                    nonce: Some(tx.nonce),
                    chain_id: tx.chain_id,
                    access_list: tx
                        .access_list
                        .clone()
                        .unwrap_or_default()
                        .iter()
                        .map(|access| {
                            (
                                access.address,
                                access
                                    .storage_keys
                                    .iter()
                                    .map(|k| U256::from_be_bytes(**k))
                                    .collect(),
                            )
                        })
                        .collect(),
                    gas_priority_fee: tx.max_priority_fee_per_gas.map(U256::from),
                    blob_hashes: tx.blob_versioned_hashes.clone().unwrap_or_default(),
                    max_fee_per_blob_gas: tx.max_fee_per_blob_gas.map(U256::from),
                })
            }
            Some(tx_envs)
        }
        _ => None,
    }
}
