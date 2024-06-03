# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large layer 2 blocks. All blocks are in the CANCUN spec with no dependencies, and we benchmark with `snmalloc` as the global memory allocator to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal layer 2. However, it may be representative of application-specific layer 2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | Speedup     |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ----------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 131.56 ms            | 84.405 ms          | 🟢1.56      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 224.57 ms            | 71.100 ms          | 🟢3.16      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 633.69 ms            | 60.044 ms          | 🟢**10.55** |

## Ethereum Mainnet Blocks

This benchmark includes several transactions for each Ethereum hardfork that alters the EVM spec. We include blocks with high parallelism, highly inter-dependent blocks, and some random blocks to ensure we benchmark against all scenarios. It is also a good testing platform for aggressively running blocks to find race conditions if there are any.

The current hardcoded concurrency level is 8, which has performed best for Ethereum blocks thus far. Increasing it will improve results for blocks with more parallelism but hurt small or highly interdependent blocks due to thread overheads. Ideally, our static analysis will be smart enough to auto-tune this better.

To run the benchmark:

```sh
$ cargo bench --bench mainnet
```

To benchmark with profiling for development (preferably after commenting out the sequential run):

```sh
# Higher level with flamegraph
$ CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --bench mainnet -- --bench

# Lower level with perf
$ CARGO_PROFILE_BENCH_DEBUG=true cargo bench --bench mainnet
$ perf record target/release/deps/mainnet-??? --bench
$ perf report
```

Blocks marked with `--` are small blocks that PEVM falls back to sequential execution. Spawning scoped threads and dropping the multi-version data structure alone may take longer than executing the whole block sequentially. Currently, these blocks either:

- Have fewer than two transactions.
- Use less than 378,000 gas.

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | --                   | --                 | --         |
| 930196       | FRONTIER        | 18               | 378,000    | --                   | --                 | --         |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 81.068 µs            | 107.48 µs          | 🔴0.75     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 495.50 µs            | 874.72 µs          | 🔴**0.57** |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.8071 ms            | 1.7637 ms          | 🟢1.02     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 227.87 µs            | 363.67 µs          | 🔴0.63     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 116.99 µs            | 109.74 µs          | 🟢1.07     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 99.555 µs            | 98.988 µs          | 🟢1.01     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 967.24 µs            | 486.30 µs          | 🟢1.99     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 764.09 µs            | 356.33 µs          | 🟢2.14     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6266 ms            | 2.3409 ms          | 🟢1.12     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 765.26 µs            | 1.3385 ms          | 🔴0.57     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.0747 ms            | 1.3167 ms          | 🟢3.09     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9064 ms            | 2.5143 ms          | 🟢1.95     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.1057 ms            | 1.1632 ms          | 🟢2.67     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 773.01 µs            | 924.99 µs          | 🔴0.84     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5769 ms            | 2.9287 ms          | 🟢1.56     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.3761 ms            | 2.1448 ms          | 🔴0.64     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2157 ms            | 3.0763 ms          | 🟢2.02     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.048 ms            | 8.4350 ms          | 🟢1.31     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 2.0133 ms            | 2.6535 ms          | 🔴0.76     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.3027 ms            | 3.8634 ms          | 🔴0.85     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.9387 ms            | 1.9615 ms          | 🟢2.01     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.591 ms            | 10.325 ms          | 🟢1.22     |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.261 ms            | 8.1239 ms          | 🟢2.99     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.5271 ms            | 5.2355 ms          | 🟢1.63     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.4007 ms            | 5.7527 ms          | 🔴0.59     |
| 14029313     | LONDON          | 724              | 30,074,554 | 9.4821 ms            | 2.6161 ms          | 🟢**3.62** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.973 ms            | 6.6564 ms          | 🟢1.95     |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.054 ms            | 5.7383 ms          | 🟢2.45     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.5134 ms            | 5.6237 ms          | 🔴0.8      |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.9477 ms            | 3.7823 ms          | 🟢2.63     |
| 15537393     | LONDON          | 1                | 29,991,429 | --                   | --                 | --         |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0444 ms            | 1.9747 ms          | 🟢1.54     |
| 15538827     | MERGE           | 823              | 29,981,465 | 12.644 ms            | 6.2018 ms          | 🟢2.04     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.8846 ms            | 2.9784 ms          | 🟢2.98     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3238 ms            | 2.6458 ms          | 🟢2.01     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.243 ms            | 9.2127 ms          | 🟢1.55     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.761 ms            | 8.4718 ms          | 🟢1.86     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.062 ms            | 9.0847 ms          | 🟢1.22     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1936 ms            | 1.3476 ms          | 🟢1.63     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9909 ms            | 6.7874 ms          | 🟢1.47     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.004 ms            | 8.7209 ms          | 🟢2.41     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 9.0268 ms            | 4.4856 ms          | 🟢2.01     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3388 ms            | 1.1151 ms          | 🟢1.2      |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1087 ms            | 2.9146 ms          | 🟢1.75     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.589 ms            | 6.2094 ms          | 🟢1.71     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.129 ms            | 6.8509 ms          | 🟢1.77     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.217 ms            | 7.9867 ms          | 🟢1.65     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 888.17 µs            | 623.25 µs          | 🟢1.43     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.3086 ms            | 3.9840 ms          | 🟢1.58     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.611 ms            | 4.1008 ms          | 🟢2.59     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4322 ms            | 1.5354 ms          | 🟢1.58     |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.62** for a large block with few dependencies.
- The **max slow down is x0.57** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
