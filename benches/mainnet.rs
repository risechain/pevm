//! Benchemark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{num::NonZeroUsize, thread};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::{execute_revm, get_block_env, get_block_spec, get_tx_envs};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

pub fn criterion_benchmark(c: &mut Criterion) {
    let concurrency_level = thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        // 8 seems to be the sweet max for Ethereum blocks. Any more
        // will yield many overheads and hurt execution on (small) blocks
        // with many dependencies.
        .min(NonZeroUsize::new(8).unwrap());

    common::for_each_block_from_disk(|block, db| {
        let spec_id = get_block_spec(&block.header).unwrap();
        let block_env = get_block_env(&block.header, None).unwrap();
        let tx_envs = get_tx_envs(&block.transactions).unwrap();

        let mut group = c.benchmark_group(format!(
            "Block {}({} txs, {} gas)",
            block.header.number.unwrap(),
            block.transactions.len(),
            block.header.gas_used
        ));
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
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
