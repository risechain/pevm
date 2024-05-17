//! Benchemark mocked blocks that exceed 1 Gigagas.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use alloy_primitives::{Address, U160, U256};
use block_stm_revm::execute_revm;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use revm::primitives::{BlockEnv, SpecId, TransactTo, TxEnv};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

const GIGA_GAS: u64 = 1_000_000_000;
const RAW_TRANSFER_GAS: u64 = 21_000;

pub fn benchmark_gigagas(c: &mut Criterion) {
    let block_size = (GIGA_GAS as f64 / RAW_TRANSFER_GAS as f64).ceil() as usize;
    // Mock the beneficiary account (`Address:ZERO`) and the next `block_size` user accounts.
    let db = common::build_inmem_db((0..=block_size).map(common::mock_account));
    let spec_id = SpecId::LATEST;
    let block_env = BlockEnv::default();
    // Independent senders that send to themselves.
    let tx_envs = (1..=block_size)
        .map(|i| {
            let address = Address::from(U160::from(i));
            TxEnv {
                caller: address,
                transact_to: TransactTo::Call(address),
                value: U256::from(1),
                gas_limit: RAW_TRANSFER_GAS,
                gas_price: U256::from(1),
                ..TxEnv::default()
            }
        })
        .collect::<Vec<_>>();
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);

    let mut group = c.benchmark_group("Independent Raw Transfers");
    group.bench_function("Sequential", |b| {
        b.iter(|| {
            common::execute_sequential(
                black_box(db.clone()),
                black_box(spec_id),
                black_box(block_env.clone()),
                black_box(&tx_envs),
            )
        })
    });
    group.bench_function("Parallel", |b| {
        b.iter(|| {
            execute_revm(
                black_box(db.clone()),
                black_box(spec_id),
                black_box(block_env.clone()),
                black_box(tx_envs.clone()),
                black_box(concurrency_level),
            )
        })
    });
    group.finish();
}

criterion_group!(benches, benchmark_gigagas);
criterion_main!(benches);
