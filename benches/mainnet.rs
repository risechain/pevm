//! Benchemark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::chain::PevmEthereum;

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

#[global_allocator]
static GLOBAL: rpmalloc::RpMalloc = rpmalloc::RpMalloc;

pub fn criterion_benchmark(c: &mut Criterion) {
    let chain = PevmEthereum::mainnet();

    common::for_each_block_from_disk(|block, storage| {
        let concurrency_level = thread::available_parallelism()
            .unwrap_or(NonZeroUsize::MIN)
            .min(
                // Excessive threads can lead to unnecessary overhead and negatively impact performance.
                // TODO: fine tune this condition
                NonZeroUsize::new(if block.transactions.len() <= 140 {
                    8
                } else {
                    13
                })
                .unwrap(),
            );

        let mut group = c.benchmark_group(format!(
            "Block {} ({} txs, {} gas)",
            block.header.number.unwrap(),
            block.transactions.len(),
            block.header.gas_used
        ));
        group.bench_function("Sequential", |b| {
            b.iter(|| {
                pevm::execute(
                    black_box(&storage),
                    black_box(&chain),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(true),
                )
            })
        });
        group.bench_function("Parallel", |b| {
            b.iter(|| {
                pevm::execute(
                    black_box(&storage),
                    black_box(&chain),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(false),
                )
            })
        });
        group.finish();
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
