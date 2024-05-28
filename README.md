# RISE Parallel EVM

![CI](https://github.com/risechain/pevm/actions/workflows/ci.yml/badge.svg)

Blazingly fast Parallel EVM in Rust.

:warning: This repository is a **work in progress** and is **not production ready** :construction:

![Banner](./assets/banner.jpg)

RISE PEVM is **a parallel execution engine for EVM chain transactions** heavily inspired by [Block-STM](https://arxiv.org/abs/2203.06871). Since Blockchain transactions are inherently sequential, a parallel execution engine must detect dependencies and avoid conflicts to guarantee the same deterministic outcome with sequential execution. Block-STM optimistically executes transactions and re-executes when conflicts arise using a collaborative scheduler and a multi-version shared data structure. Since it does not require prior knowledge or constraints on the input transactions, **replacing an existing sequential executor with Block-STM is easy for substantial performance boosts**.

Block-STM was initially designed for the Aptos blockchain that runs MoveVM. We must consider several modifications to make it work well with EVM. For instance, all EVM transactions in the same block read and write to the beneficiary account for gas payment, making all transactions interdependent by default. We must carefully monitor reads to this beneficiary account to lazily evaluate it at the end of the block or when an explicit read arises. Polygon has already adapted a version of Block-STM for EVM in their Go node. Our implementation is written in Rust, specifically on [revm](https://github.com/bluealloy/revm), to aim for even higher performance, especially when parallel execution in Go is still slower than sequential execution in Rust! These performance improvements are critical to syncing chains with a massive state, building blocks for low-block-time chains, and ZK provers.

Finally, while Aptos and Polygon embed their Block-STM implementation directly into their nodes, **this dedicated repository provides both robust versions and a playground for further advancements**. For instance, we can introduce static-analysed metadata from an optimised mempool, support multiple underlying executors, track read checkpoints to re-execute from instead of re-executing the whole transaction upon conflicts and hyper-optimise the implementation at low system levels.

## Goals

- Become the fastest EVM (block) execution engine for rapid block building and syncing.
- Provide deep tests and audits to guarantee safety and support new developments.
- Provide deep benchmarks to showcase improvements and support new developments.
- Complete a robust version for syncing and building blocks for Ethereum, RISE, Optimism, and more EVM chains.
- Get integrated into Ethereum clients and ZK provers like [Reth](https://github.com/paradigmxyz/reth), [Helios](https://github.com/a16z/helios), and [Zeth](https://github.com/risc0/zeth) to help make the Ethereum ecosystem blazingly fast.

## Development

### Alpha TODO

- Complete a vital test suite.
- More low-hanging fruit optimizations.
- Robust error handling.
- Better structure, types, and API for integration.
- Benchmark a [Reth](https://github.com/paradigmxyz/reth) integration for syncing and building Ethereum and RISE blocks.

### Beta TODO

- Write custom memory allocators for the whole execution phase and the multi-version data structure.
- Add pre-provided metadata from a statically analysed mempool or upstream nodes.
- Track read checkpoints to re-execute from there instead of re-executing the whole transaction upon conflicts.
- Hyper-optimise the implementation at low system levels.
- Support multiple EVM executors (REVM, JIT & AOT compilers, etc.).

### Testing

```bash
$ git submodule update --init
# Our tests are heavy, avoid running them in parallel to not risk nuking RAM.
# Each parallel test still executes parallelly up to the number of CPUs anyway.
$ cargo test --release -- --test-threads=1
```

## Benchmarks

See the dedicated doc [here](./benches/README.md).
