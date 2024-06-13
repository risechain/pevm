//! Benchemark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

pub fn criterion_benchmark(c: &mut Criterion) {
    let concurrency_level = thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        // 8 seems to be the sweet max for Ethereum blocks. Any more
        // will yield many overheads and hurt execution on (small) blocks
        // with many dependencies.
        .min(NonZeroUsize::new(8).unwrap());

    common::for_each_block_from_disk(|block, storage| {
        let mut group = c.benchmark_group(format!(
            "Block {}({} txs, {} gas)",
            block.header.number.unwrap(),
            block.transactions.len(),
            block.header.gas_used
        ));
        group.bench_function("Sequential", |b| {
            b.iter(|| {
                pevm::execute(
                    pevm::Network::Ethereum,
                    black_box(storage.clone()),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(true),
                )
            })
        });
        group.bench_function("Parallel", |b| {
            b.iter(|| {
                pevm::execute(
                    pevm::Network::Ethereum,
                    black_box(storage.clone()),
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
