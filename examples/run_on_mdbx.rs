//! Check mainnet blocks using [OnDiskStorage]
//! For help, run: `cargo run --example run_on_mdbx -- --help`

#![allow(missing_docs)]

use std::{
    fs::File,
    io::{BufReader, Error},
};

use alloy_rpc_types::Block;
use clap::Parser;
use pevm::{chain::PevmEthereum, OnDiskStorage, StorageWrapper};
use revm::db::CacheDB;

#[path = "../tests/common/mod.rs"]
pub mod common;

/// Check mainnet blocks using [OnDiskStorage]
#[derive(Parser, Debug)]
#[clap(name = "run_on_mdbx")]
struct Args {
    /// Path to MDBX dir
    #[clap(long, value_name = "DIR")]
    mdbx: String,
    /// Path to block.json file
    #[clap(long, value_name = "FILE")]
    block: String,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let block: Block = {
        let file = File::open(args.block)?;
        serde_json::from_reader(BufReader::new(file))?
    };

    let on_disk_storage = OnDiskStorage::open(args.mdbx).map_err(Error::other)?;
    let wrapped_storage = StorageWrapper(&on_disk_storage);
    let db = CacheDB::new(&wrapped_storage);

    let chain = PevmEthereum::mainnet();
    common::test_execute_alloy(&db, &chain, block, true);

    Ok(())
}
