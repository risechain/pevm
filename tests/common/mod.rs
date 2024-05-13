// https://doc.rust-lang.org/book/ch11-03-test-organization.html
pub mod utils;
pub mod builders {
    pub mod contract;
    pub mod storage;
}

// for backward compatibility
pub use utils::mock_account;
pub use utils::test_txs;
