use super::precompiles::RisePrecompiles;
use super::{
    RiseContext, RiseHaltReason, RiseTransaction, RiseTransactionError, handler::RiseHandler,
};
use revm::{
    Database, ExecuteEvm,
    context::{BlockEnv, ContextError, ContextSetters, Evm, FrameStack, TxEnv},
    context_interface::{
        ContextTr,
        result::{EVMError, ExecResultAndState, ExecutionResult},
    },
    handler::{
        EthFrame, EvmTr, FrameInitOrResult, FrameResult, Handler, ItemOrResult, evm::FrameTr,
        instructions::EthInstructions,
    },
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
};

pub(crate) type RiseError<DB> = EVMError<<DB as Database>::Error, RiseTransactionError>;

/// RISE EVM wrapping [`Evm`] with RISE-specific precompiles and handler dispatch.
#[derive(Debug)]
#[allow(clippy::type_complexity)]
pub struct RiseEvm<DB: Database>(
    Evm<
        RiseContext<DB>,
        (),
        EthInstructions<EthInterpreter, RiseContext<DB>>,
        RisePrecompiles,
        EthFrame<EthInterpreter>,
    >,
);

impl<DB: Database> RiseEvm<DB> {
    pub(crate) fn new(ctx: RiseContext<DB>) -> Self {
        let spec = *ctx.cfg().spec();
        Self(Evm {
            ctx,
            inspector: (),
            instruction: EthInstructions::new_mainnet_with_spec(spec),
            precompiles: RisePrecompiles::default(),
            frame_stack: FrameStack::new_prealloc(8),
        })
    }
}

impl<DB: Database> EvmTr for RiseEvm<DB> {
    type Context = RiseContext<DB>;
    type Instructions = EthInstructions<EthInterpreter, RiseContext<DB>>;
    type Precompiles = RisePrecompiles;
    type Frame = EthFrame<EthInterpreter>;

    fn all(
        &self,
    ) -> (
        &Self::Context,
        &Self::Instructions,
        &Self::Precompiles,
        &FrameStack<Self::Frame>,
    ) {
        self.0.all()
    }

    fn all_mut(
        &mut self,
    ) -> (
        &mut Self::Context,
        &mut Self::Instructions,
        &mut Self::Precompiles,
        &mut FrameStack<Self::Frame>,
    ) {
        self.0.all_mut()
    }

    fn frame_init(
        &mut self,
        frame_input: <Self::Frame as FrameTr>::FrameInit,
    ) -> Result<ItemOrResult<&mut Self::Frame, FrameResult>, ContextError<DB::Error>> {
        self.0.frame_init(frame_input)
    }

    fn frame_run(&mut self) -> Result<FrameInitOrResult<Self::Frame>, ContextError<DB::Error>> {
        self.0.frame_run()
    }

    fn frame_return_result(
        &mut self,
        result: FrameResult,
    ) -> Result<Option<FrameResult>, ContextError<DB::Error>> {
        self.0.frame_return_result(result)
    }
}

impl<DB: Database> ExecuteEvm for RiseEvm<DB> {
    type Tx = RiseTransaction<TxEnv>;
    type Block = BlockEnv;
    type State = EvmState;
    type Error = RiseError<DB>;
    type ExecutionResult = ExecutionResult<RiseHaltReason>;

    fn set_block(&mut self, block: Self::Block) {
        self.0.ctx.set_block(block);
    }

    fn transact_one(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        RiseHandler::default().run(self)
    }

    fn finalize(&mut self) -> Self::State {
        self.0.ctx.journal_mut().finalize()
    }

    fn replay(
        &mut self,
    ) -> Result<ExecResultAndState<Self::ExecutionResult, Self::State>, Self::Error> {
        RiseHandler::default()
            .run(self)
            .map(|result| ExecResultAndState::new(result, self.finalize()))
    }
}
