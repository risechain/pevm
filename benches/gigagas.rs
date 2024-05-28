//! Benchemark mocked blocks that exceed 1 Gigagas.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use alloy_primitives::{Address, U160, U256};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::execute_revm;
use revm::{
    db::PlainAccount,
    primitives::{BlockEnv, SpecId, TransactTo, TxEnv},
    InMemoryDB,
};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

#[path = "../tests/erc20/mod.rs"]
pub mod erc20;

#[path = "../tests/uniswap/mod.rs"]
pub mod uniswap;

const GIGA_GAS: u64 = 1_000_000_000;

#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

pub fn bench(c: &mut Criterion, name: &str, db: InMemoryDB, txs: Vec<TxEnv>) {
    let concurrency_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let spec_id = SpecId::LATEST;
    let block_env = BlockEnv::default();
    let mut group = c.benchmark_group(name);
    group.bench_function("Sequential", |b| {
        b.iter(|| {
            common::execute_sequential(
                black_box(db.clone()),
                black_box(spec_id),
                black_box(block_env.clone()),
                black_box(&txs),
            )
        })
    });
    group.bench_function("Parallel", |b| {
        b.iter(|| {
            execute_revm(
                black_box(db.clone()),
                black_box(spec_id),
                black_box(block_env.clone()),
                black_box(txs.clone()),
                black_box(concurrency_level),
            )
        })
    });
    group.finish();
}

pub fn bench_raw_transfers(c: &mut Criterion) {
    let gas_limit = 21_000;
    let block_size = (GIGA_GAS as f64 / gas_limit as f64).ceil() as usize;
    bench(
        c,
        "Independent Raw Transfers",
        common::build_inmem_db((0..=block_size).map(common::mock_account)),
        (1..=block_size)
            .map(|i| {
                let address = Address::from(U160::from(i));
                TxEnv {
                    caller: address,
                    transact_to: TransactTo::Call(address),
                    value: U256::from(1),
                    gas_limit,
                    gas_price: U256::from(1),
                    ..TxEnv::default()
                }
            })
            .collect::<Vec<_>>(),
    );
}

pub fn bench_erc20(c: &mut Criterion) {
    let block_size = (GIGA_GAS as f64 / erc20::GAS_LIMIT as f64).ceil() as usize;
    let (mut state, txs) = erc20::generate_cluster(block_size, 1, 1);
    state.push((Address::ZERO, PlainAccount::default())); // Beneficiary
    bench(c, "Independent ERC20", common::build_inmem_db(state), txs);
}

pub fn bench_uniswap(c: &mut Criterion) {
    let block_size = (GIGA_GAS as f64 / uniswap::GAS_LIMIT as f64).ceil() as usize;
    let mut final_state = vec![(Address::ZERO, PlainAccount::default())]; // Beneficiary
    let mut final_txs = Vec::<TxEnv>::new();
    for _ in 0..block_size {
        let (state, txs) = uniswap::generate_cluster(1, 1);
        final_state.extend(state);
        final_txs.extend(txs);
    }
    bench(
        c,
        "Independent Uniswap",
        common::build_inmem_db(final_state),
        final_txs,
    );
}

pub fn benchmark_gigagas(c: &mut Criterion) {
    bench_raw_transfers(c);
    bench_erc20(c);
    bench_uniswap(c);
}

criterion_group!(benches, benchmark_gigagas);
criterion_main!(benches);
