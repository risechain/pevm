//! Vendored OP-Stack EVM types, stripped down for RISE.
//!
//! The external `op-revm` crate pins `revm = "^38"` which is incompatible with our revm 39
//! upgrade.  We vendor only what RISE needs, updated for the revm 39 API and simplified to
//! remove all L1/DA/operator fee arithmetic (RISE zeroes all of these).

pub(crate) mod evm;
pub(crate) mod handler;
pub(crate) mod precompiles;
pub(crate) mod transaction;

pub(crate) use evm::RiseEvm;
pub(crate) use transaction::{RiseTransaction, RiseTransactionError};

use revm::{
    Context, Journal,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{Cfg, ContextTr, JournalTr, result::HaltReason},
    primitives::{Address, address, hardfork::SpecId},
    state::EvmState,
};

pub(crate) const L1_FEE_RECIPIENT: Address = address!("0x420000000000000000000000000000000000001A");
pub(crate) const OPERATOR_FEE_RECIPIENT: Address =
    address!("0x420000000000000000000000000000000000001B");
pub(crate) const BASE_FEE_RECIPIENT: Address =
    address!("0x4200000000000000000000000000000000000019");

/// The default OP context type: mainnet context + [`RiseTransaction`] + unit chain (no L1 fees).
pub(crate) type OpContext<DB> =
    Context<BlockEnv, RiseTransaction<TxEnv>, CfgEnv<SpecId>, DB, Journal<DB>, ()>;

/// Marker trait for OP-capable EVM contexts.
pub(crate) trait OpContextTr:
    ContextTr<
        Journal: JournalTr<State = EvmState>,
        Tx = RiseTransaction<TxEnv>,
        Cfg: Cfg<Spec = SpecId>,
        Chain = (),
    >
{
}

impl<T> OpContextTr for T where
    T: ContextTr<
            Journal: JournalTr<State = EvmState>,
            Tx = RiseTransaction<TxEnv>,
            Cfg: Cfg<Spec = SpecId>,
            Chain = (),
        >
{
}

/// Halt reason for RISE/OP-Stack execution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RiseHaltReason {
    Base(HaltReason),
    FailedDeposit,
}

impl From<HaltReason> for RiseHaltReason {
    fn from(value: HaltReason) -> Self {
        Self::Base(value)
    }
}
