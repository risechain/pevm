//! Benchmark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::{chain::PevmEthereum, Pevm, StorageWrapper};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

// [rpmalloc] is generally better but can crash on AWS Graviton.
#[cfg(target_arch = "aarch64")]
#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
#[cfg(not(target_arch = "aarch64"))]
#[global_allocator]
static GLOBAL: rpmalloc::RpMalloc = rpmalloc::RpMalloc;

pub fn criterion_benchmark(c: &mut Criterion) {
    let chain = PevmEthereum::mainnet();
    let concurrency_level = thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        // This max should be tuned to the running machine,
        // ideally also per block depending on the number of
        // transactions, gas usage, etc. ARM machines seem to
        // go higher thanks to their low thread overheads.
        .min(
            NonZeroUsize::new(
                #[cfg(target_arch = "aarch64")]
                12,
                #[cfg(not(target_arch = "aarch64"))]
                8,
            )
            .unwrap(),
        );
    let mut pevm = Pevm::default();

    common::for_each_block_from_disk(|block, storage| {
        let db = StorageWrapper(&storage);
        let mut group = c.benchmark_group(format!(
            "Block {}({} txs, {} gas)",
            block.header.number,
            block.transactions.len(),
            block.header.gas_used
        ));
        group.bench_function("Sequential", |b| {
            b.iter(|| {
                pevm.execute(
                    black_box(&db),
                    black_box(&chain),
                    black_box(block.clone()),
                    black_box(concurrency_level),
                    black_box(true),
                )
            })
        });
        group.bench_function("Parallel", |b| {
            b.iter(|| {
                pevm.execute(
                    black_box(&db),
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
