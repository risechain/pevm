use revm::{
    context::TxEnv,
    context_interface::transaction::Transaction,
    handler::SystemCallTx,
    primitives::{Address, B256, Bytes, TxKind, U256},
};

/// Deposit transaction type byte.
pub(crate) const DEPOSIT_TRANSACTION_TYPE: u8 = 0x7E;

/// Parts of a deposit transaction not present in normal transactions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DepositTransactionParts {
    pub source_hash: B256,
    pub mint: Option<u128>,
    pub is_system_transaction: bool,
}

impl DepositTransactionParts {
    pub const fn new(source_hash: B256, mint: Option<u128>, is_system_transaction: bool) -> Self {
        Self {
            source_hash,
            mint,
            is_system_transaction,
        }
    }
}

/// Optimism transaction: wraps a base transaction with deposit-specific fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiseTransaction<T: Transaction> {
    pub base: T,
    /// Enveloped EIP-2718 bytes, required for L1 cost computation on non-deposits.
    pub enveloped_tx: Option<Bytes>,
    pub deposit: DepositTransactionParts,
}

impl<T: Transaction> RiseTransaction<T> {
    pub fn new(base: T) -> Self {
        Self {
            base,
            enveloped_tx: None,
            deposit: DepositTransactionParts::default(),
        }
    }

    pub const fn enveloped_tx(&self) -> Option<&Bytes> {
        self.enveloped_tx.as_ref()
    }

    pub const fn mint(&self) -> Option<u128> {
        self.deposit.mint
    }

    pub const fn is_system_transaction(&self) -> bool {
        self.deposit.is_system_transaction
    }

    pub fn is_deposit(&self) -> bool {
        self.tx_type() == DEPOSIT_TRANSACTION_TYPE
    }
}

impl<T: Transaction> AsRef<T> for RiseTransaction<T> {
    fn as_ref(&self) -> &T {
        &self.base
    }
}

impl Default for RiseTransaction<TxEnv> {
    fn default() -> Self {
        Self {
            base: TxEnv::default(),
            // Dummy non-empty bytes so validate_env passes the MissingEnvelopedTx check
            // for non-deposit transactions when the EVM is first constructed.
            enveloped_tx: Some(vec![0x00].into()),
            deposit: DepositTransactionParts::default(),
        }
    }
}

impl<TX: Transaction + SystemCallTx> SystemCallTx for RiseTransaction<TX> {
    fn new_system_tx_with_caller(
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Self {
        let mut tx = Self::new(TX::new_system_tx_with_caller(
            caller,
            system_contract_address,
            data,
        ));
        tx.enveloped_tx = Some(Bytes::default());
        tx
    }
}

impl<T: Transaction> Transaction for RiseTransaction<T> {
    type AccessListItem<'a>
        = T::AccessListItem<'a>
    where
        T: 'a;
    type Authorization<'a>
        = T::Authorization<'a>
    where
        T: 'a;

    fn tx_type(&self) -> u8 {
        // Deposits are identified by a non-zero source_hash.
        if self.deposit.source_hash == B256::ZERO {
            self.base.tx_type()
        } else {
            DEPOSIT_TRANSACTION_TYPE
        }
    }

    fn caller(&self) -> Address {
        self.base.caller()
    }
    fn gas_limit(&self) -> u64 {
        self.base.gas_limit()
    }
    fn value(&self) -> U256 {
        self.base.value()
    }
    fn input(&self) -> &Bytes {
        self.base.input()
    }
    fn nonce(&self) -> u64 {
        self.base.nonce()
    }
    fn kind(&self) -> TxKind {
        self.base.kind()
    }
    fn chain_id(&self) -> Option<u64> {
        self.base.chain_id()
    }
    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        self.base.max_priority_fee_per_gas()
    }
    fn max_fee_per_gas(&self) -> u128 {
        self.base.max_fee_per_gas()
    }
    fn gas_price(&self) -> u128 {
        self.base.gas_price()
    }
    fn blob_versioned_hashes(&self) -> &[B256] {
        self.base.blob_versioned_hashes()
    }
    fn max_fee_per_blob_gas(&self) -> u128 {
        self.base.max_fee_per_blob_gas()
    }
    fn authorization_list_len(&self) -> usize {
        self.base.authorization_list_len()
    }

    fn access_list(&self) -> Option<impl Iterator<Item = Self::AccessListItem<'_>>> {
        self.base.access_list()
    }

    fn effective_gas_price(&self, base_fee: u128) -> u128 {
        // Deposits use gas_price directly (always 0).
        if self.tx_type() == DEPOSIT_TRANSACTION_TYPE {
            return self.gas_price();
        }
        self.base.effective_gas_price(base_fee)
    }

    fn authorization_list(&self) -> impl Iterator<Item = Self::Authorization<'_>> {
        self.base.authorization_list()
    }
}

/// Optimism transaction validation error.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RiseTransactionError {
    Base(revm::context::result::InvalidTransaction),
    DepositSystemTxPostRegolith,
    HaltedDepositPostRegolith,
    MissingEnvelopedTx,
}

impl revm::context_interface::transaction::TransactionError for RiseTransactionError {}

impl core::fmt::Display for RiseTransactionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Base(e) => e.fmt(f),
            Self::DepositSystemTxPostRegolith => f.write_str(
                "deposit system transactions post regolith hardfork are not supported",
            ),
            Self::HaltedDepositPostRegolith => f.write_str(
                "deposit transaction halted post-regolith; error will be bubbled up to main return handler",
            ),
            Self::MissingEnvelopedTx => f.write_str(
                "missing enveloped transaction bytes for non-deposit transaction",
            ),
        }
    }
}

impl core::error::Error for RiseTransactionError {}

impl From<revm::context::result::InvalidTransaction> for RiseTransactionError {
    fn from(value: revm::context::result::InvalidTransaction) -> Self {
        Self::Base(value)
    }
}

impl<DBError> From<RiseTransactionError>
    for revm::context_interface::result::EVMError<DBError, RiseTransactionError>
{
    fn from(value: RiseTransactionError) -> Self {
        Self::Transaction(value)
    }
}
