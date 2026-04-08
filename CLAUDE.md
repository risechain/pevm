# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Engineering Principles

- **Fundamental**: Seek deep understanding of the problem space and underlying concepts. Make decisions from first principles, not patterns or assumptions.
- **Correctness**: Never compromise correctness even if it results in less elegant and slower code. Always preserve invariants and safety.
- **Simplicity**: Seek the most obvious solutions. Avoid clever abstractions, unnecessary layers, or overengineering.
- **Minimalism**: Do exactly what is required for the task. No extra features nor speculative generalization. Keep code and diffs the bare minimum, as long as it is readable and not hacky.
- **Low Runtime Friction**: Avoid unnecessary clones, channels, data serialization, and especially external calls. Reuse heap allocations where possible and generally don't pay for what we don't use.
- **Observability**: Emit meaningful logs and metrics on critical paths for ease of debugging. However, do not overdo it and spam noise.

## Commands

```bash
# Build
cargo build
cargo build --release

# Lint & format
cargo clippy --workspace --all-targets
cargo fmt --all
taplo fmt --option reorder_keys=true   # TOML formatting

# Tests (must use --test-threads=1 to avoid resource contention)
git submodule update --init            # Required once: pulls ethereum/tests submodule
cargo test --workspace --release -- --test-threads=1

# Run a single test
cargo test --workspace --release <test_name> -- --test-threads=1

# Benchmarks
cargo bench --features global-alloc --bench mainnet
cargo bench --features global-alloc --bench gigagas
```

## Architecture

**pevm** is a parallel EVM engine for Ethereum and OP-EVM chains (RISE, Base, etc.). It executes a block's transactions concurrently while producing identical results to sequential execution.

### Workspace

- `crates/pevm/` — core library
- `bins/fetch/` — CLI to fetch and snapshot Ethereum blocks for testing

### Core execution model (Block-STM optimistic parallelism)

1. **Scheduler** (`scheduler.rs`) — distributes execution and validation tasks across rayon threads; tracks transaction dependencies.
2. **VM** (`vm.rs`) — wraps `revm`; executes one transaction per call; records which memory locations were read/written.
3. **Multi-version memory** (`mv_memory.rs`) — stores all write sets indexed by `(location, tx_index, incarnation)`; lets each transaction read the latest prior write without locking.
4. **Pevm** (`pevm.rs`) — top-level orchestrator; sets up the multi-version memory, spawns worker threads via `std::thread::scope`, collects receipts, applies block rewards.
5. **Storage** (`storage/`) — abstraction over chain state; backends: `InMemoryStorage` (tests) and `RpcStorage` (live chain, feature-gated).
6. **Chain** (`chain/`) — per-chain policies: `PevmEthereum` for Ethereum mainnet, `PevmOptimism` for OP-EVM chains (RISE, Base, and any other OP Stack chain). Handles block env construction, tx parsing, and reward application.

### Key design choices

- **Lazy updates**: raw ETH transfers are written as `MemoryValue::LazyRecipient` (addition) or `MemoryValue::LazySender` (subtraction) instead of absolute balances, reducing artificial dependencies between transactions. Lazy addresses are tracked and fully evaluated at the end of the block.
- **Conflict detection**: after each transaction executes, a validation pass checks that its read set still matches current multi-version memory. On conflict the transaction is re-executed with an incremented incarnation.
- **Identity hashing**: `TxIdx` and memory-location hashes use suffix/identity hashers to avoid redundant hashing in hot paths.

### Testing layout (`crates/pevm/tests/`)

- `ethereum/` — general state tests from the official `ethereum/tests` submodule
- `evm/` — mocked blocks (raw transfers, ERC-20, Uniswap, mixed, beneficiary edge cases)
- `mainnet/` — snapshots of real Ethereum mainnet blocks
- `common/` — shared test harness (`runner.rs`, `storage.rs`, `snapshot_data.rs`)

### Features

| Feature | Enables |
|---------|---------|
| `optimism` | Optimism L2 chain support |
| `rpc-storage` | `RpcStorage` backend |
| `full` | Both of the above |
| `global-alloc` | Custom allocators (snmalloc/rpmalloc/jemalloc) — use for benchmarks |
