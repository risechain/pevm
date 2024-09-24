//! Benchmark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::{chain::PevmEthereum, Pevm, PevmStrategy};

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
    let mut pevm = Pevm::default();

    common::for_each_block_from_disk(|block, storage| {
        let mut group = c.benchmark_group(format!(
            "Block {}({} txs, {} gas)",
            block.header.number,
            block.transactions.len(),
            block.header.gas_used
        ));
        group.bench_function("Sequential", |b| {
            b.iter(|| {
                assert!(pevm
                    .execute(
                        black_box(&storage),
                        black_box(&chain),
                        black_box(block.clone()),
                        black_box(PevmStrategy::sequential()),
                    )
                    .is_ok());
            })
        });
        group.bench_function("Parallel", |b| {
            b.iter(|| {
                assert!(pevm
                    .execute(
                        black_box(&storage),
                        black_box(&chain),
                        black_box(block.clone()),
                        black_box(PevmStrategy::auto(
                            block.transactions.len(),
                            block.header.gas_used,
                        )),
                    )
                    .is_ok());
            })
        });
        group.finish();
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
