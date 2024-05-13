// https://doc.rust-lang.org/book/ch11-03-test-organization.html
pub mod runner;
pub mod storage;

// for backward compatibility
pub use runner::mock_account;
pub use runner::test_txs;
