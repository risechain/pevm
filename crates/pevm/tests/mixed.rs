//! Test raw transfers -- A block with random raw transfers, ERC-20 transfers, and Uniswap swaps.

use pevm::{Bytecodes, ChainState, EvmAccount, InMemoryStorage};
use rand::random;
use revm::primitives::{env::TxEnv, Address, TransactTo, U256};

pub mod common;
pub mod erc20;
pub mod uniswap;

#[test]
fn mixed_block() {
    let target_block_size = 100_000; // number of transactions

    // TODO: Run a few times
    let mut block_size = 0;
    let mut final_state = ChainState::default();
    final_state.insert(Address::ZERO, EvmAccount::default()); // Beneficiary
    let mut final_bytecodes = Bytecodes::default();
    let mut final_txs = Vec::new();
    // 1 to 10
    let small_random = || (random::<u8>() % 10 + 1) as usize;
    while block_size < target_block_size {
        match small_random() % 3 {
            0 => {
                // Raw transfers are more popular
                let no_txs = random::<u16>();
                for _ in 0..no_txs {
                    let (address, account) = common::mock_account(small_random());
                    final_state.insert(address, account);
                    final_txs.push(TxEnv {
                        caller: address,
                        transact_to: TransactTo::Call(address), // TODO: Randomize for tighter test
                        value: U256::from(1),
                        gas_limit: common::RAW_TRANSFER_GAS_LIMIT,
                        gas_price: U256::from(1),
                        ..TxEnv::default()
                    });
                }
                block_size += no_txs as usize;
            }
            1 => {
                let (state, bytecodes, txs) =
                    erc20::generate_cluster(small_random(), small_random(), small_random());
                block_size += txs.len();
                final_state.extend(state);
                final_bytecodes.extend(bytecodes);
                final_txs.extend(txs);
            }
            _ => {
                let (state, bytecodes, txs) =
                    uniswap::generate_cluster(small_random(), small_random());
                block_size += txs.len();
                final_state.extend(state);
                final_bytecodes.extend(bytecodes);
                final_txs.extend(txs);
            }
        }
    }
    common::test_execute_revm(
        InMemoryStorage::new(final_state, Some(&final_bytecodes), []),
        // TODO: Shuffle transactions to scatter dependencies around the block.
        // Note that we'll need to guarantee that the nonces are increasing.
        final_txs,
    );
}
