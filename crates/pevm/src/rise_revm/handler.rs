use crate::rise_revm::{
    BASE_FEE_RECIPIENT, L1_FEE_RECIPIENT, OPERATOR_FEE_RECIPIENT, OpContextTr, RiseHaltReason,
    transaction::{DEPOSIT_TRANSACTION_TYPE, RiseTransactionError},
};
use revm::{
    context::{
        LocalContextTr,
        journaled_state::{JournalCheckpoint, account::JournaledAccountTr},
    },
    context_interface::{
        Block, Cfg, ContextTr, JournalTr, Transaction,
        context::take_error,
        result::{EVMError, ExecutionResult, FromStringError, ResultGas},
    },
    handler::{
        EthFrame, EvmTr, Handler, MainnetHandler,
        evm::FrameTr,
        handler::EvmTrError,
        post_execution::{self, reimburse_caller},
        pre_execution::{calculate_caller_fee, validate_account_nonce_and_code_with_components},
    },
    interpreter::{Gas, InitialAndFloorGas, interpreter::EthInterpreter},
    primitives::U256,
};
use std::vec::Vec;

/// Helper to identify transaction-level errors (used in `catch_error`).
pub(crate) trait IsTxError {
    fn is_tx_error(&self) -> bool;
}

impl<DB, TX> IsTxError for EVMError<DB, TX> {
    fn is_tx_error(&self) -> bool {
        matches!(self, Self::Transaction(_))
    }
}

/// Wraps [`MainnetHandler`] and overrides the methods that differ for OP Stack chains:
/// deposit transaction handling, gas accounting, and fee distribution.
#[derive(Debug)]
pub(crate) struct OpHandler<EVM, ERROR>(MainnetHandler<EVM, ERROR, EthFrame<EthInterpreter>>);

impl<EVM, ERROR> Default for OpHandler<EVM, ERROR> {
    fn default() -> Self {
        Self(MainnetHandler::default())
    }
}

impl<EVM, ERROR> Handler for OpHandler<EVM, ERROR>
where
    EVM: EvmTr<Context: OpContextTr, Frame = EthFrame<EthInterpreter>>,
    ERROR: EvmTrError<EVM> + From<RiseTransactionError> + FromStringError + IsTxError,
{
    type Evm = EVM;
    type Error = ERROR;
    type HaltReason = RiseHaltReason;

    fn validate_env(&self, evm: &mut Self::Evm) -> Result<(), Self::Error> {
        let ctx = evm.ctx();
        let tx = ctx.tx();

        if tx.tx_type() == DEPOSIT_TRANSACTION_TYPE {
            // System transactions are rejected post-Regolith; RISE is always post-Regolith.
            if tx.is_system_transaction() {
                return Err(RiseTransactionError::DepositSystemTxPostRegolith.into());
            }
            return Ok(());
        }

        // Non-deposits must carry enveloped bytes for L1 cost computation.
        if tx.enveloped_tx().is_none() {
            return Err(RiseTransactionError::MissingEnvelopedTx.into());
        }

        self.0.validate_env(evm)
    }

    fn validate_against_state_and_deduct_caller(
        &self,
        evm: &mut Self::Evm,
        _init_and_floor_gas: &mut InitialAndFloorGas,
    ) -> Result<(), Self::Error> {
        let (block, tx, cfg, journal, _, _) = evm.ctx().all_mut();

        if tx.tx_type() == DEPOSIT_TRANSACTION_TYPE {
            let basefee = block.basefee() as u128;
            let blob_price = block.blob_gasprice().unwrap_or_default();
            let mut caller = journal.load_account_with_code_mut(tx.caller())?.data;

            // Deposits have gas_price=0, so effective_balance_spending = value; net deduction is 0.
            // Compute explicitly to match op-revm behaviour.
            let effective_balance_spending = tx
                .effective_balance_spending(basefee, blob_price)
                .expect("deposit effective balance spending overflow")
                - tx.value();

            let mut new_balance = caller
                .balance()
                .saturating_add(U256::from(tx.mint().unwrap_or_default()))
                .saturating_sub(effective_balance_spending);

            if cfg.is_balance_check_disabled() {
                new_balance = new_balance.max(tx.value());
            }

            caller.set_balance(new_balance);
            if tx.kind().is_call() {
                caller.bump_nonce();
            }
            return Ok(());
        }

        let mut caller_account = journal.load_account_with_code_mut(tx.caller())?.data;
        validate_account_nonce_and_code_with_components(&caller_account.account().info, tx, cfg)?;
        // L1 cost is always zero on RISE — no additional deduction needed.
        let balance = calculate_caller_fee(caller_account.account().info.balance, tx, block, cfg)?;
        caller_account.set_balance(balance);
        if tx.kind().is_call() {
            caller_account.bump_nonce();
        }

        Ok(())
    }

    fn last_frame_result(
        &mut self,
        evm: &mut Self::Evm,
        _original_reservoir: u64,
        frame_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        let tx_gas_limit = evm.ctx().tx().gas_limit();

        let instruction_result = frame_result.interpreter_result().result;
        let gas = frame_result.gas_mut();
        let remaining = gas.remaining();
        let refunded = gas.refunded();
        let reservoir = gas.reservoir();
        let state_gas_spent = gas.state_gas_spent();

        // Spend the full gas limit; reservoir and state_gas_spent are saved above and restored below.
        *gas = Gas::new_spent_with_reservoir(tx_gas_limit, 0);

        // RISE is always post-Regolith: return unused gas on success/revert for all tx types.
        if instruction_result.is_ok() {
            gas.erase_cost(remaining);
            gas.record_refund(refunded);
        } else if instruction_result.is_revert() {
            gas.erase_cost(remaining);
        }

        // Restore fields that Gas::new_spent_with_reservoir overwrites.
        gas.set_state_gas_spent(state_gas_spent);
        gas.set_reservoir(reservoir);

        Ok(())
    }

    fn reimburse_caller(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        // Operator fee refund — always zero on RISE.
        reimburse_caller(evm.ctx(), frame_result.gas(), U256::ZERO).map_err(From::from)
    }

    fn refund(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
        eip7702_refund: i64,
    ) {
        frame_result.gas_mut().record_refund(eip7702_refund);
        // Always post-Regolith and post-London: apply EIP-3529 capped refund for all tx types.
        let _ = evm;
        frame_result.gas_mut().set_final_refund(true);
    }

    fn reward_beneficiary(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        if evm.ctx().tx().tx_type() == DEPOSIT_TRANSACTION_TYPE {
            return Ok(());
        }

        // Pay the sequencer (coinbase) its share via the mainnet path.
        self.0.reward_beneficiary(evm, frame_result)?;

        let basefee = evm.ctx().block().basefee() as u128;
        let effective_used = frame_result
            .gas()
            .used()
            .saturating_sub(frame_result.gas().reservoir());
        let base_fee_amount = U256::from(basefee.saturating_mul(effective_used as u128));

        // RISE disables DA footprint and operator fees. Still touch these accounts
        // to match revm's sequential execution state.
        let journal = evm.ctx().journal_mut();
        for (recipient, amount) in [
            (L1_FEE_RECIPIENT, U256::ZERO),
            (BASE_FEE_RECIPIENT, base_fee_amount),
            (OPERATOR_FEE_RECIPIENT, U256::ZERO),
        ] {
            journal.balance_incr(recipient, amount)?;
        }

        Ok(())
    }

    fn execution_result(
        &mut self,
        evm: &mut Self::Evm,
        frame_result: <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
        result_gas: ResultGas,
    ) -> Result<ExecutionResult<Self::HaltReason>, Self::Error> {
        take_error::<Self::Error, _>(evm.ctx().error())?;

        let exec_result = post_execution::output(evm.ctx(), frame_result, result_gas)
            .map_haltreason(RiseHaltReason::Base);

        // RISE is always post-Regolith: a halted deposit is always a fatal error.
        if exec_result.is_halt() && evm.ctx().tx().tx_type() == DEPOSIT_TRANSACTION_TYPE {
            return Err(ERROR::from(RiseTransactionError::HaltedDepositPostRegolith));
        }
        evm.ctx().journal_mut().commit_tx();
        evm.ctx().local_mut().clear();
        evm.frame_stack().clear();

        Ok(exec_result)
    }

    fn catch_error(
        &self,
        evm: &mut Self::Evm,
        error: Self::Error,
    ) -> Result<ExecutionResult<Self::HaltReason>, Self::Error> {
        let is_deposit = evm.ctx().tx().tx_type() == DEPOSIT_TRANSACTION_TYPE;
        let is_tx_error = error.is_tx_error();
        let mut output = Err(error);

        if is_tx_error && is_deposit {
            let caller = evm.ctx().tx().caller();
            let mint = evm.ctx().tx().mint();
            let gas_limit = evm.ctx().tx().gas_limit();
            let journal = evm.ctx().journal_mut();

            journal.checkpoint_revert(JournalCheckpoint::default());

            let mut acc = journal.load_account_mut(caller)?;
            acc.bump_nonce();
            acc.incr_balance(U256::from(mint.unwrap_or_default()));
            drop(acc); // release borrow before commit_tx

            journal.commit_tx();

            // RISE is always post-Regolith: failed deposits always consume their full gas.
            output = Ok(ExecutionResult::Halt {
                reason: RiseHaltReason::FailedDeposit,
                gas: ResultGas::default().with_total_gas_spent(gas_limit),
                logs: Vec::new(),
            });
        }

        evm.ctx().local_mut().clear();
        evm.frame_stack().clear();

        output
    }
}
