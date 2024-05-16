//! Benchemark mainnet blocks with needed state loaded in memory.

// TODO: More fancy benchmarks & plots.

#![allow(missing_docs)]

use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    num::NonZeroUsize,
    thread,
};

use alloy_primitives::Address;
use alloy_rpc_types::Block;
use block_stm_revm::{execute_revm, get_block_env, get_block_spec, get_tx_envs};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use revm::db::PlainAccount;

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

pub fn criterion_benchmark(c: &mut Criterion) {
    let concunrrecy_level = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);

    for block_path in fs::read_dir("blocks").unwrap() {
        let block_path = block_path.unwrap().path();
        let block_number = block_path.file_name().unwrap().to_str().unwrap();

        // Parse block
        let block: Block = serde_json::from_reader(BufReader::new(
            File::open(format!("blocks/{block_number}/block.json")).unwrap(),
        ))
        .unwrap();
        let spec_id = get_block_spec(&block.header).unwrap();
        let block_env = get_block_env(&block.header, None).unwrap();
        let tx_envs = get_tx_envs(&block.transactions).unwrap();

        // Parse state
        let accounts: HashMap<Address, PlainAccount> = serde_json::from_reader(BufReader::new(
            File::open(format!("blocks/{block_number}/state_for_execution.json")).unwrap(),
        ))
        .unwrap();
        let db = common::build_inmem_db(accounts);

        let mut group = c.benchmark_group(format!(
            "Block {block_number}({} txs, {} gas)",
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
                    black_box(concunrrecy_level),
                )
            })
        });
        group.finish();
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
