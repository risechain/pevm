use crate::{frame::EthFrame, instructions::EthInstructions, EthPrecompiles};
use context::{BlockEnv, Cfg, CfgEnv, Context, Evm, FrameStack, Journal, TxEnv};
use context_interface::{Block, Database, JournalTr, Transaction};
use database_interface::EmptyDB;
use interpreter::interpreter::EthInterpreter;
use primitives::hardfork::SpecId;

/// Type alias for a mainnet EVM instance with standard Ethereum components.
pub type MainnetEvm<CTX, INSP = ()> =
    Evm<CTX, INSP, EthInstructions<EthInterpreter, CTX>, EthPrecompiles, EthFrame<EthInterpreter>>;

/// Type alias for a mainnet context with standard Ethereum environment types.
pub type MainnetContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB, Journal<DB>, ()>;

/// Trait for building mainnet EVM instances from contexts.
pub trait MainBuilder: Sized {
    /// The context type that will be used in the EVM.
    type Context;

    /// Builds a mainnet EVM instance without an inspector.
    fn build_mainnet(self) -> MainnetEvm<Self::Context>;

    /// Builds a mainnet EVM instance with the provided inspector.
    fn build_mainnet_with_inspector<INSP>(self, inspector: INSP)
        -> MainnetEvm<Self::Context, INSP>;
}

impl<BLOCK, TX, CFG, DB, JOURNAL, CHAIN> MainBuilder for Context<BLOCK, TX, CFG, DB, JOURNAL, CHAIN>
where
    BLOCK: Block,
    TX: Transaction,
    CFG: Cfg,
    DB: Database,
    JOURNAL: JournalTr<Database = DB>,
{
    type Context = Self;

    fn build_mainnet(self) -> MainnetEvm<Self::Context> {
        let spec = self.cfg.spec().into();
        Evm {
            ctx: self,
            inspector: (),
            instruction: EthInstructions::new_mainnet_with_spec(spec),
            precompiles: EthPrecompiles::new(spec),
            frame_stack: FrameStack::new_prealloc(8),
        }
    }

    fn build_mainnet_with_inspector<INSP>(
        self,
        inspector: INSP,
    ) -> MainnetEvm<Self::Context, INSP> {
        let spec = self.cfg.spec().into();
        Evm {
            ctx: self,
            inspector,
            instruction: EthInstructions::new_mainnet_with_spec(spec),
            precompiles: EthPrecompiles::new(spec),
            frame_stack: FrameStack::new_prealloc(8),
        }
    }
}

/// Trait used to initialize Context with default mainnet types.
pub trait MainContext {
    /// Creates a new mainnet context with default configuration.
    fn mainnet() -> Self;
}

impl MainContext for Context<BlockEnv, TxEnv, CfgEnv, EmptyDB, Journal<EmptyDB>, ()> {
    fn mainnet() -> Self {
        Context::new(EmptyDB::new(), SpecId::default())
    }
}
