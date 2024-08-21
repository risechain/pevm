//! Benchemark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{
    num::NonZeroUsize,
    thread,
    time::{Duration, Instant},
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::{chain::PevmEthereum, OnDiskStorage};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

#[global_allocator]
static GLOBAL: rpmalloc::RpMalloc = rpmalloc::RpMalloc;

pub fn criterion_benchmark(c: &mut Criterion) {
    let chain = PevmEthereum::mainnet();
    let concurrency_level = thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        // 8 seems to be the sweet max for Ethereum blocks. Any more
        // will yield many overheads and hurt execution on (small) blocks
        // with many dependencies.
        .min(NonZeroUsize::new(12).unwrap());

    common::for_each_block_from_disk(|block, in_memory_storage, mdbx_dir| {
        let mut group = c.benchmark_group(format!(
            "Block {}({} txs, {} gas)",
            block.header.number.unwrap(),
            block.transactions.len(),
            block.header.gas_used
        ));
        group.bench_function("Sequential/In Memory", |b| {
            b.iter(|| {
                pevm::execute(
                    black_box(&in_memory_storage),
                    black_box(&chain),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(true),
                )
            })
        });
        group.bench_function("Parallel/In Memory", |b| {
            b.iter(|| {
                pevm::execute(
                    black_box(&in_memory_storage),
                    black_box(&chain),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(false),
                )
            })
        });
        group.bench_function("Sequential/On Disk", |b| {
            b.iter_custom(|iters| {
                let mut total_duration = Duration::ZERO;
                for _i in 0..iters {
                    let on_disk_storage = OnDiskStorage::open(mdbx_dir).unwrap();
                    let start = Instant::now();
                    pevm::execute(
                        black_box(&on_disk_storage),
                        black_box(&chain),
                        black_box(block.clone()),
                        black_box(concurrency_level),
                        black_box(true),
                    )
                    .unwrap();
                    total_duration += start.elapsed();
                }
                total_duration
            })
        });

        group.bench_function("Parallel/On Disk", |b| {
            b.iter_custom(|iters| {
                let mut total_duration = Duration::ZERO;
                for _i in 0..iters {
                    let on_disk_storage = OnDiskStorage::open(mdbx_dir).unwrap();
                    let start = Instant::now();
                    pevm::execute(
                        black_box(&on_disk_storage),
                        black_box(&chain),
                        black_box(block.clone()),
                        black_box(concurrency_level),
                        black_box(false),
                    )
                    .unwrap();
                    total_duration += start.elapsed();
                }
                total_duration
            })
        });

        group.finish();
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
