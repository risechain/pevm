use crate::rise_revm::{
    BASE_FEE_RECIPIENT, L1_FEE_RECIPIENT, OPERATOR_FEE_RECIPIENT, RiseHaltReason,
    evm::RiseEvm,
    transaction::{DEPOSIT_TRANSACTION_TYPE, RiseTransactionError},
};
use revm::{
    Database,
    context::{
        LocalContextTr,
        journaled_state::{JournalCheckpoint, account::JournaledAccountTr},
    },
    context_interface::{
        Block, ContextTr, JournalTr, Transaction,
        context::take_error,
        result::{EVMError, ExecutionResult, ResultGas},
    },
    handler::{
        EthFrame, EvmTr, FrameResult, Handler, MainnetHandler,
        post_execution::{self, reimburse_caller},
        pre_execution::{calculate_caller_fee, validate_account_nonce_and_code_with_components},
    },
    interpreter::{Gas, InitialAndFloorGas, interpreter::EthInterpreter},
    primitives::U256,
};
use std::vec::Vec;

type RiseHandlerError<DB> = EVMError<<DB as Database>::Error, RiseTransactionError>;

/// Wraps [`MainnetHandler`] and overrides the methods that differ for OP Stack chains:
/// deposit transaction handling, gas accounting, and fee distribution.
#[derive(Debug)]
pub(crate) struct RiseHandler<DB: Database>(
    MainnetHandler<RiseEvm<DB>, RiseHandlerError<DB>, EthFrame<EthInterpreter>>,
);

impl<DB: Database> Default for RiseHandler<DB> {
    fn default() -> Self {
        Self(MainnetHandler::default())
    }
}

impl<DB: Database> Handler for RiseHandler<DB> {
    type Evm = RiseEvm<DB>;
    type Error = RiseHandlerError<DB>;
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
        _: &mut InitialAndFloorGas,
    ) -> Result<(), Self::Error> {
        let (block, tx, cfg, journal, _, _) = evm.ctx().all_mut();

        let mut caller = journal.load_account_with_code_mut(tx.caller())?.data;

        if tx.tx_type() == DEPOSIT_TRANSACTION_TYPE {
            // Deposit balance update: new_balance = old_balance + mint.
            // The general formula is: new_balance = old_balance + mint - effective_balance_spending + value,
            // where effective_balance_spending = gas_limit * gas_price + blob_cost + value.
            // Deposits enforce gas_price=0 and carry no blob hashes, so effective_balance_spending = value,
            // and the net deduction is always zero — leaving only the mint credit.
            let new_balance = caller
                .balance()
                .saturating_add(U256::from(tx.mint().unwrap_or_default()));
            caller.set_balance(new_balance);
        } else {
            validate_account_nonce_and_code_with_components(&caller.account().info, tx, cfg)?;
            // L1 cost is always zero on RISE — no additional deduction needed.
            caller.set_balance(calculate_caller_fee(
                caller.account().info.balance,
                tx,
                block,
                cfg,
            )?);
        }

        if tx.kind().is_call() {
            caller.bump_nonce();
        }

        Ok(())
    }

    fn last_frame_result(
        &mut self,
        evm: &mut Self::Evm,
        _: u64,
        frame_result: &mut FrameResult,
    ) -> Result<(), Self::Error> {
        let instruction_result = frame_result.interpreter_result().result;
        let gas = frame_result.gas_mut();

        // Save fields that Gas::new_spent_with_reservoir overwrites, then reset to fully-spent.
        let remaining = gas.remaining();
        let refunded = gas.refunded();
        let reservoir = gas.reservoir();
        let state_gas_spent = gas.state_gas_spent();
        *gas = Gas::new_spent_with_reservoir(evm.ctx().tx().gas_limit(), 0);

        // RISE is always post-Regolith: return unused gas on success/revert for all tx types.
        if instruction_result.is_ok() {
            gas.erase_cost(remaining);
            gas.record_refund(refunded);
        } else if instruction_result.is_revert() {
            gas.erase_cost(remaining);
        }

        gas.set_state_gas_spent(state_gas_spent);
        gas.set_reservoir(reservoir);

        Ok(())
    }

    fn reimburse_caller(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut FrameResult,
    ) -> Result<(), Self::Error> {
        // Operator fee refund — always zero on RISE.
        reimburse_caller(evm.ctx(), frame_result.gas(), U256::ZERO).map_err(From::from)
    }

    fn refund(&self, _: &mut Self::Evm, frame_result: &mut FrameResult, eip7702_refund: i64) {
        frame_result.gas_mut().record_refund(eip7702_refund);
        // Always post-Regolith and post-London: apply EIP-3529 capped refund for all tx types.
        frame_result.gas_mut().set_final_refund(true);
    }

    fn reward_beneficiary(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut FrameResult,
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
        frame_result: FrameResult,
        result_gas: ResultGas,
    ) -> Result<ExecutionResult<Self::HaltReason>, Self::Error> {
        take_error::<Self::Error, _>(evm.ctx().error())?;

        let exec_result = post_execution::output(evm.ctx(), frame_result, result_gas)
            .map_haltreason(RiseHaltReason::Base);

        // RISE is always post-Regolith: a halted deposit is always a fatal error.
        if exec_result.is_halt() && evm.ctx().tx().tx_type() == DEPOSIT_TRANSACTION_TYPE {
            return Err(RiseTransactionError::HaltedDepositPostRegolith.into());
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
        let output = if matches!(error, EVMError::Transaction(_))
            && evm.ctx().tx().tx_type() == DEPOSIT_TRANSACTION_TYPE
        {
            let caller = evm.ctx().tx().caller();
            let mint = evm.ctx().tx().mint();

            evm.ctx()
                .journal_mut()
                .checkpoint_revert(JournalCheckpoint::default());

            let mut acc = evm.ctx().journal_mut().load_account_mut(caller)?;
            acc.bump_nonce();
            acc.incr_balance(U256::from(mint.unwrap_or_default()));

            evm.ctx().journal_mut().commit_tx();

            // RISE is always post-Regolith: failed deposits always consume their full gas.
            Ok(ExecutionResult::Halt {
                reason: RiseHaltReason::FailedDeposit,
                gas: ResultGas::default().with_total_gas_spent(evm.ctx().tx().gas_limit()),
                logs: Vec::new(),
            })
        } else {
            Err(error)
        };

        evm.ctx().local_mut().clear();
        evm.frame_stack().clear();

        output
    }
}
