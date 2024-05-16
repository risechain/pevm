#![allow(missing_docs)]
// https://bheisler.github.io/criterion.rs/book/getting_started.html

use alloy_primitives::Address;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use revm::{
    db::PlainAccount,
    primitives::{BlockEnv, SpecId},
};

#[path = "../tests/common/mod.rs"]
pub mod common;

#[path = "../tests/erc20/mod.rs"]
pub mod erc20;

const SAMPLE_SIZE: usize = 40;

// we expect to have 1 gigagas
const APPROX_GAS_PER_TX: usize = 26_938;
const APPROX_BLOCK_SIZE: usize = 1_000_000_000 / APPROX_GAS_PER_TX;

pub fn benchmark_erc20_independent(c: &mut Criterion) {
    let (mut state, txs) = erc20::generate_cluster(APPROX_BLOCK_SIZE, 1, 1);
    state.push((Address::ZERO, PlainAccount::default()));
    let db = common::build_inmem_db(state);

    let mut group = c.benchmark_group("ERC20 Independent Transactions");
    group.sample_size(SAMPLE_SIZE);
    group.bench_function("Sequential", |b| {
        b.iter(|| {
            common::execute_sequential(
                black_box(db.clone()),
                black_box(SpecId::LATEST),
                black_box(BlockEnv::default()),
                black_box(&txs),
            )
        })
    });
    group.bench_function("Parallel", |b| {
        b.iter(|| {
            block_stm_revm::execute_revm(
                black_box(db.clone()),
                black_box(SpecId::LATEST),
                black_box(BlockEnv::default()),
                black_box(txs.clone()),
                black_box(
                    std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN),
                ),
            )
        })
    });
    group.finish();
}

pub fn benchmark_erc20_clustered(c: &mut Criterion) {
    let (mut state, txs) = erc20::generate_cluster(APPROX_BLOCK_SIZE / 3 / 2, 3, 2);
    state.push((Address::ZERO, PlainAccount::default()));
    let db = common::build_inmem_db(state);

    let mut group = c.benchmark_group("ERC20 Clustered Transactions");
    group.sample_size(SAMPLE_SIZE);
    group.bench_function("Sequential", |b| {
        b.iter(|| {
            common::execute_sequential(
                black_box(db.clone()),
                black_box(SpecId::LATEST),
                black_box(BlockEnv::default()),
                black_box(&txs),
            )
        })
    });
    group.bench_function("Parallel", |b| {
        b.iter(|| {
            block_stm_revm::execute_revm(
                black_box(db.clone()),
                black_box(SpecId::LATEST),
                black_box(BlockEnv::default()),
                black_box(txs.clone()),
                black_box(
                    std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN),
                ),
            )
        })
    });
    group.finish();
}

// https://docs.rs/criterion/latest/criterion/macro.criterion_main.html
criterion_group!(
    benches,
    benchmark_erc20_independent,
    benchmark_erc20_clustered
);
criterion_main!(benches);
