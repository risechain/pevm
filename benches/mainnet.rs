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
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pevm::{execute_revm, get_block_env, get_block_spec, get_tx_envs};
use revm::{db::PlainAccount, primitives::KECCAK_EMPTY};

// Better project structure
#[path = "../tests/common/mod.rs"]
pub mod common;

pub fn criterion_benchmark(c: &mut Criterion) {
    // TODO: Fine-tune concurrency level
    let concurrency_level = thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        // 8 seems to be the sweet max for Ethereum blocks. Any more
        // will yield many overheads and hurt execution on (small) blocks
        // with many dependencies.
        .min(NonZeroUsize::new(8).unwrap());

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
        let mut accounts: HashMap<Address, PlainAccount> = serde_json::from_reader(BufReader::new(
            File::open(format!("blocks/{block_number}/state_for_execution.json")).unwrap(),
        ))
        .unwrap();
        // Hacky but we don't serialize the whole account info to save space
        // So we need to resconstruct intermediate values upon deserializing.
        for (_, account) in accounts.iter_mut() {
            account.info.previous_or_original_balance = account.info.balance;
            account.info.previous_or_original_nonce = account.info.nonce;
            if let Some(code) = account.info.code.clone() {
                let code_hash = code.hash_slow();
                account.info.code_hash = code_hash;
                account.info.previous_or_original_code_hash = code_hash;
            } else {
                account.info.code_hash = KECCAK_EMPTY;
            }
        }
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
                    black_box(concurrency_level),
                )
            })
        });
        group.finish();
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
