use context_interface::{
    result::{InvalidHeader, InvalidTransaction},
    transaction::{Transaction, TransactionType},
    Block, Cfg, ContextTr,
};
use core::cmp;
use interpreter::{instructions::calculate_initial_tx_gas_for_tx, InitialAndFloorGas};
use primitives::{eip4844, hardfork::SpecId, B256};

/// Validates the execution environment including block and transaction parameters.
pub fn validate_env<CTX: ContextTr, ERROR: From<InvalidHeader> + From<InvalidTransaction>>(
    context: CTX,
) -> Result<(), ERROR> {
    let spec = context.cfg().spec().into();
    // `prevrandao` is required for the merge
    if spec.is_enabled_in(SpecId::MERGE) && context.block().prevrandao().is_none() {
        return Err(InvalidHeader::PrevrandaoNotSet.into());
    }
    // `excess_blob_gas` is required for Cancun
    if spec.is_enabled_in(SpecId::CANCUN) && context.block().blob_excess_gas_and_price().is_none() {
        return Err(InvalidHeader::ExcessBlobGasNotSet.into());
    }
    validate_tx_env::<CTX>(context, spec).map_err(Into::into)
}

/// Validate legacy transaction gas price against basefee.
#[inline]
pub fn validate_legacy_gas_price(
    gas_price: u128,
    base_fee: Option<u128>,
) -> Result<(), InvalidTransaction> {
    // Gas price must be at least the basefee.
    if let Some(base_fee) = base_fee {
        if gas_price < base_fee {
            return Err(InvalidTransaction::GasPriceLessThanBasefee);
        }
    }
    Ok(())
}

/// Validate transaction that has EIP-1559 priority fee
pub fn validate_priority_fee_tx(
    max_fee: u128,
    max_priority_fee: u128,
    base_fee: Option<u128>,
    disable_priority_fee_check: bool,
) -> Result<(), InvalidTransaction> {
    if !disable_priority_fee_check && max_priority_fee > max_fee {
        // Or gas_max_fee for eip1559
        return Err(InvalidTransaction::PriorityFeeGreaterThanMaxFee);
    }

    // Check minimal cost against basefee
    if let Some(base_fee) = base_fee {
        let effective_gas_price = cmp::min(max_fee, base_fee.saturating_add(max_priority_fee));
        if effective_gas_price < base_fee {
            return Err(InvalidTransaction::GasPriceLessThanBasefee);
        }
    }

    Ok(())
}

/// Validate priority fee for transactions that support EIP-1559 (Eip1559, Eip4844, Eip7702).
#[inline]
fn validate_priority_fee_for_tx<TX: Transaction>(
    tx: TX,
    base_fee: Option<u128>,
    disable_priority_fee_check: bool,
) -> Result<(), InvalidTransaction> {
    validate_priority_fee_tx(
        tx.max_fee_per_gas(),
        tx.max_priority_fee_per_gas().unwrap_or_default(),
        base_fee,
        disable_priority_fee_check,
    )
}

/// Validate EIP-4844 transaction.
pub fn validate_eip4844_tx(
    blobs: &[B256],
    max_blob_fee: u128,
    block_blob_gas_price: u128,
    max_blobs: Option<u64>,
) -> Result<(), InvalidTransaction> {
    // Ensure that the user was willing to at least pay the current blob gasprice
    if block_blob_gas_price > max_blob_fee {
        return Err(InvalidTransaction::BlobGasPriceGreaterThanMax {
            block_blob_gas_price,
            tx_max_fee_per_blob_gas: max_blob_fee,
        });
    }

    // There must be at least one blob
    if blobs.is_empty() {
        return Err(InvalidTransaction::EmptyBlobs);
    }

    // All versioned blob hashes must start with VERSIONED_HASH_VERSION_KZG
    for blob in blobs {
        if blob[0] != eip4844::VERSIONED_HASH_VERSION_KZG {
            return Err(InvalidTransaction::BlobVersionNotSupported);
        }
    }

    // Ensure the total blob gas spent is at most equal to the limit
    // assert blob_gas_used <= MAX_BLOB_GAS_PER_BLOCK
    if let Some(max_blobs) = max_blobs {
        if blobs.len() > max_blobs as usize {
            return Err(InvalidTransaction::TooManyBlobs {
                have: blobs.len(),
                max: max_blobs as usize,
            });
        }
    }
    Ok(())
}

/// Validate transaction against block and configuration for mainnet.
pub fn validate_tx_env<CTX: ContextTr>(
    context: CTX,
    spec_id: SpecId,
) -> Result<(), InvalidTransaction> {
    // Check if the transaction's chain id is correct
    let tx = context.tx();
    let tx_type = tx.tx_type();

    let base_fee = if context.cfg().is_base_fee_check_disabled() {
        None
    } else {
        Some(context.block().basefee() as u128)
    };

    let tx_type = TransactionType::from(tx_type);

    // Check chain_id if config is enabled.
    // EIP-155: Simple replay attack protection
    if context.cfg().tx_chain_id_check() {
        if let Some(chain_id) = tx.chain_id() {
            if chain_id != context.cfg().chain_id() {
                return Err(InvalidTransaction::InvalidChainId);
            }
        } else if !tx_type.is_legacy() && !tx_type.is_custom() {
            // Legacy transaction are the only one that can omit chain_id.
            return Err(InvalidTransaction::MissingChainId);
        }
    }

    // tx gas cap is not enforced if state gas is enabled.
    if !context.cfg().is_amsterdam_eip8037_enabled() {
        // EIP-7825: Transaction Gas Limit Cap
        let cap = context.cfg().tx_gas_limit_cap();
        if tx.gas_limit() > cap {
            return Err(InvalidTransaction::TxGasLimitGreaterThanCap {
                gas_limit: tx.gas_limit(),
                cap,
            });
        }
    }

    let disable_priority_fee_check = context.cfg().is_priority_fee_check_disabled();

    match tx_type {
        TransactionType::Legacy => {
            validate_legacy_gas_price(tx.gas_price(), base_fee)?;
        }
        TransactionType::Eip2930 => {
            // Enabled in BERLIN hardfork
            if !spec_id.is_enabled_in(SpecId::BERLIN) {
                return Err(InvalidTransaction::Eip2930NotSupported);
            }
            validate_legacy_gas_price(tx.gas_price(), base_fee)?;
        }
        TransactionType::Eip1559 => {
            if !spec_id.is_enabled_in(SpecId::LONDON) {
                return Err(InvalidTransaction::Eip1559NotSupported);
            }
            validate_priority_fee_for_tx(tx, base_fee, disable_priority_fee_check)?;
        }
        TransactionType::Eip4844 => {
            if !spec_id.is_enabled_in(SpecId::CANCUN) {
                return Err(InvalidTransaction::Eip4844NotSupported);
            }

            validate_priority_fee_for_tx(tx, base_fee, disable_priority_fee_check)?;

            validate_eip4844_tx(
                tx.blob_versioned_hashes(),
                tx.max_fee_per_blob_gas(),
                context.block().blob_gasprice().unwrap_or_default(),
                context.cfg().max_blobs_per_tx(),
            )?;
        }
        TransactionType::Eip7702 => {
            // Check if EIP-7702 transaction is enabled.
            if !spec_id.is_enabled_in(SpecId::PRAGUE) {
                return Err(InvalidTransaction::Eip7702NotSupported);
            }

            validate_priority_fee_for_tx(tx, base_fee, disable_priority_fee_check)?;

            let auth_list_len = tx.authorization_list_len();
            // The transaction is considered invalid if the length of authorization_list is zero.
            if auth_list_len == 0 {
                return Err(InvalidTransaction::EmptyAuthorizationList);
            }
        }
        TransactionType::Custom => {
            // Custom transaction type check is not done here.
        }
    };

    // Check if gas_limit is more than block_gas_limit
    // TODO(eip8037) should we enforce to `min(tx.gas_limit(), 16M) < block.gas_limit`?
    // This would enforce that regular gas is constrained.
    if !context.cfg().is_block_gas_limit_disabled() && tx.gas_limit() > context.block().gas_limit()
    {
        return Err(InvalidTransaction::CallerGasLimitMoreThanBlock);
    }

    // EIP-3860: Limit and meter initcode. Still valid with EIP-7907 and increase of initcode size.
    if spec_id.is_enabled_in(SpecId::SHANGHAI)
        && tx.kind().is_create()
        && tx.input().len() > context.cfg().max_initcode_size()
    {
        return Err(InvalidTransaction::CreateInitCodeSizeLimit);
    }

    // Check that the transaction's nonce is not at the maximum value.
    // Incrementing the nonce would overflow. Can't happen in the real world.
    if tx.nonce() == u64::MAX {
        return Err(InvalidTransaction::NonceOverflowInTransaction);
    }

    Ok(())
}

/// Validate initial transaction gas.
pub fn validate_initial_tx_gas(
    tx: impl Transaction,
    spec: SpecId,
    is_eip7623_disabled: bool,
    is_amsterdam_eip8037_enabled: bool,
    tx_gas_limit_cap: u64,
) -> Result<InitialAndFloorGas, InvalidTransaction> {
    let mut gas = calculate_initial_tx_gas_for_tx(&tx, spec);

    if is_eip7623_disabled {
        gas.floor_gas = 0
    }

    // Additional check to see if limit is big enough to cover initial gas.
    if gas.initial_total_gas > tx.gas_limit() {
        return Err(InvalidTransaction::CallGasCostMoreThanGasLimit {
            gas_limit: tx.gas_limit(),
            initial_gas: gas.initial_total_gas,
        });
    }

    // EIP-7623: Increase calldata cost
    // floor gas should be less than gas limit.
    if spec.is_enabled_in(SpecId::PRAGUE) && gas.floor_gas > tx.gas_limit() {
        return Err(InvalidTransaction::GasFloorMoreThanGasLimit {
            gas_floor: gas.floor_gas,
            gas_limit: tx.gas_limit(),
        });
    };

    // EIP-8037: Regular gas is capped at TX_MAX_GAS_LIMIT.
    // Validate that both intrinsic regular gas and floor gas fit within the cap.
    // State gas is excluded — it uses its own reservoir.
    if is_amsterdam_eip8037_enabled && tx.gas_limit() > tx_gas_limit_cap {
        let min_regular_gas = gas.initial_regular_gas().max(gas.floor_gas);
        if min_regular_gas > tx_gas_limit_cap {
            return Err(InvalidTransaction::GasFloorMoreThanGasLimit {
                gas_floor: min_regular_gas,
                gas_limit: tx_gas_limit_cap,
            });
        }
    }

    Ok(gas)
}
