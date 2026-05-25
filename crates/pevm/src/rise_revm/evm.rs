use super::precompiles::OpPrecompiles;
use super::{OpContext, OpContextTr, RiseHaltReason, RiseTransactionError, handler::OpHandler};
use revm::{
    Database, ExecuteEvm,
    context::{Cfg, ContextError, ContextSetters, Evm, FrameStack},
    context_interface::{
        ContextTr, JournalTr,
        result::{EVMError, ExecResultAndState, ExecutionResult},
    },
    handler::{
        EthFrame, EvmTr, FrameInitOrResult, Handler, ItemOrResult, PrecompileProvider,
        evm::FrameTr,
        instructions::{EthInstructions, InstructionProvider},
    },
    interpreter::{InterpreterResult, interpreter::EthInterpreter},
    primitives::hardfork::SpecId,
    state::EvmState,
};

pub(crate) type OpError<DB> = EVMError<<DB as Database>::Error, RiseTransactionError>;

/// RISE EVM wrapping [`Evm`] with RISE-specific precompiles and handler dispatch.
#[derive(Debug)]
pub struct RiseEvm<CTX>(
    Evm<CTX, (), EthInstructions<EthInterpreter, CTX>, OpPrecompiles, EthFrame<EthInterpreter>>,
);

impl<DB: Database> RiseEvm<OpContext<DB>>
where
    OpContext<DB>: ContextTr<Db = DB>,
    <OpContext<DB> as ContextTr>::Cfg: Cfg<Spec = SpecId>,
{
    pub(crate) fn new(ctx: OpContext<DB>) -> Self {
        let spec = ctx.cfg().spec();
        Self(Evm {
            ctx,
            inspector: (),
            instruction: EthInstructions::new_mainnet_with_spec(spec),
            precompiles: OpPrecompiles::default(),
            frame_stack: FrameStack::new_prealloc(8),
        })
    }
}

impl<DB: Database> EvmTr for RiseEvm<OpContext<DB>>
where
    OpContext<DB>: ContextTr<Db = DB>,
    EthInstructions<EthInterpreter, OpContext<DB>>:
        InstructionProvider<Context = OpContext<DB>, InterpreterTypes = EthInterpreter>,
    OpPrecompiles: PrecompileProvider<OpContext<DB>, Output = InterpreterResult>,
{
    type Context = OpContext<DB>;
    type Instructions = EthInstructions<EthInterpreter, OpContext<DB>>;
    type Precompiles = OpPrecompiles;
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
    ) -> Result<
        ItemOrResult<&mut Self::Frame, <Self::Frame as FrameTr>::FrameResult>,
        ContextError<DB::Error>,
    > {
        self.0.frame_init(frame_input)
    }

    fn frame_run(&mut self) -> Result<FrameInitOrResult<Self::Frame>, ContextError<DB::Error>> {
        self.0.frame_run()
    }

    fn frame_return_result(
        &mut self,
        result: <Self::Frame as FrameTr>::FrameResult,
    ) -> Result<Option<<Self::Frame as FrameTr>::FrameResult>, ContextError<DB::Error>> {
        self.0.frame_return_result(result)
    }
}

impl<DB: Database> ExecuteEvm for RiseEvm<OpContext<DB>>
where
    OpContext<DB>: ContextTr<Db = DB> + OpContextTr + ContextSetters,
    EthInstructions<EthInterpreter, OpContext<DB>>:
        InstructionProvider<Context = OpContext<DB>, InterpreterTypes = EthInterpreter>,
    OpPrecompiles: PrecompileProvider<OpContext<DB>, Output = InterpreterResult>,
{
    type Tx = <OpContext<DB> as ContextTr>::Tx;
    type Block = <OpContext<DB> as ContextTr>::Block;
    type State = EvmState;
    type Error = OpError<DB>;
    type ExecutionResult = ExecutionResult<RiseHaltReason>;

    fn set_block(&mut self, block: Self::Block) {
        self.0.ctx.set_block(block);
    }

    fn transact_one(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        OpHandler::default().run(self)
    }

    fn finalize(&mut self) -> Self::State {
        self.0.ctx.journal_mut().finalize()
    }

    fn replay(
        &mut self,
    ) -> Result<ExecResultAndState<Self::ExecutionResult, Self::State>, Self::Error> {
        OpHandler::<_, _>::default().run(self).map(|result| {
            let state = self.finalize();
            ExecResultAndState::new(result, state)
        })
    }
}
