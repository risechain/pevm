# RISE Parallel EVM

![CI](https://github.com/risechain/pevm/actions/workflows/ci.yml/badge.svg)

Blazingly fast Parallel EVM in Rust.

:warning: This repository is a **work in progress** and is **not production ready** :construction:

![Banner](./assets/banner.jpg)

**RISE pevm** is **a parallel execution engine for EVM transactions** heavily inspired by [Block-STM](https://arxiv.org/abs/2203.06871). Since Blockchain transactions are inherently sequential, a parallel execution engine must detect dependencies and conflicts to guarantee the same deterministic outcome with sequential execution. Block-STM optimistically executes transactions and re-executes when conflicts arise using a collaborative scheduler and a shared multi-version data structure. Since it does not require prior knowledge or constraints on the input transactions, **replacing an existing sequential executor with Block-STM is easy for substantial performance boosts**.

Block-STM was initially designed for the Aptos blockchain that runs MoveVM. We must consider several modifications to make it work well with EVM. For instance, all EVM transactions in the same block read and write to the beneficiary account for gas payment, making all transactions interdependent by default. We must carefully monitor reads to this beneficiary account to lazily evaluate it at the end of the block or when an explicit read arises.

While Polygon has adapted a version of Block-STM for EVM in Go, it is still slower than sequential execution in Rust and C++. On the other hand, our redesign and Rust implementation have achieved the fastest execution speed of any public EVM executor.

Finally, while Aptos and Polygon embed their pevm implementation directly into their nodes, **this dedicated repository provides robust versions and a playground for further advancements**. For instance, we can introduce static-analysed metadata from an optimised mempool, support multiple underlying executors, track read checkpoints to re-execute from upon conflicts and hyper-optimise the implementation at low system levels.

## Goals

- **Become the fastest EVM (block) execution engine** for rapid block building and syncing — **10 Gigagas/s and beyond**.
- Provide deep tests and audits to guarantee safety and support new developments.
- Provide deep benchmarks to showcase improvements and support new developments.
- Complete a robust version for syncing and building blocks for Ethereum, RISE, Optimism, and more EVM chains.
- Get integrated into Ethereum clients and ZK provers like [Reth](https://github.com/paradigmxyz/reth), [Helios](https://github.com/a16z/helios), and [Zeth](https://github.com/risc0/zeth) to help make the Ethereum ecosystem blazingly fast.

## Development

> :warning: **Warning**
> pevm is performing poorly in recent Linux kernel versions. We noticed huge performance degradation after updating a machine to Ubuntu 24.04 with Linux kernel 6.8. The current suspect is the new EEVDF scheduler, which does not go well with pevm's scheduler & thread management. Until we fully fix the issue, it is advised to **build and run pevm on Linux kernel 6.5**.

- Install [cmake](https://cmake.org) to build `snmalloc` (a highly performant memory allocator).

### Alpha Done

- Build a Block-STM foundation to improve on.
- Lazily update gas payments to the beneficiary account as implicit reads & writes.
- Lazily update raw transfer senders & recipients as implicit reads & writes.
- Improve scheduler design & aggressively find tasks to save scheduling cycles.
- Many low-level optimisations.
- Complete foundation test & benchmark suites.

### Alpha TODO

- Complete OP & RISE support.
- Lazily update ERC-20 transfers.
- Robust error handling.
- Better types and API for integration.
- More low-hanging fruit optimisations.
- Complete a [Reth](https://github.com/paradigmxyz/reth) integration for syncing and building Ethereum and RISE blocks.

### Future Plans

- Optimise concurrent data structures to maximise CPU cache and stack memory.
- Optimise the scheduler & worker threads to minimise synchronization.
- Add pre-provided metadata (DAG, states to preload, etc.) from a statically analysed mempool and upstream nodes.
- Custom memory allocators for the whole execution phase and the multi-version data structure.
- Track read checkpoints to re-execute from instead of re-executing the whole transaction upon conflicts.
- Support multiple EVM executors (REVM, JIT & AOT compilers, etc.).
- Hyper-optimise at low system levels (kernel configurations, writing hot paths in Assembly, etc.).
- Propose an EIP to "tax" blocks with low parallelism.

```sh
$ cargo build
```

### Tooling

Fetcher

We provide a command-line interface (CLI) tool to snapshot the state of a block. This tool fetches a block from an RPC provider and snapshots the state to disk.

```sh
$ cargo run --bin fetch <BLOCK_ID> <RPC_URL>
```

Where `<BLOCK_ID>` may be a hash or a number.

## Testing

We have three test groups:

- [ethereum/tests](https://github.com/ethereum/tests)'s [general state tests](tests/ethereum/main.rs).
- Mocked blocks: [raw transfers](tests/raw_transfers.rs), [erc20](tests/erc20/main.rs), [uniswap](tests/uniswap/main.rs), [mixed](tests/mixed.rs), [beneficiary](tests/beneficiary.rs), and [small blocks](tests/small_blocks.rs).
- [Ethereum mainnet blocks](tests/mainnet.rs).

```sh
$ git submodule update --init
# Running our heavy tests in parallel would congest resources.
# Each test still executes parallelly anyway.
$ cargo test --release -- --test-threads=1
```

### Benchmarks

See the dedicated doc [here](./benches/README.md).
