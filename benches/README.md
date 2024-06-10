# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | Speedup     |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ----------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 125.53 ms            | 86.209 ms          | 🟢1.46      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 219.55 ms            | 73.418 ms          | 🟢2.99      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 619.28 ms            | 59.854 ms          | 🟢**10.35** |

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

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | 2.0311 µs            | 1.9987 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 26.237 µs            | 26.244 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 69.629 µs            | 69.694 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 365.47 µs            | 401.17 µs          | 🔴0.91     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.4322 ms            | 1.4211 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 165.34 µs            | 176.90 µs          | 🔴0.93     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 101.22 µs            | 110.16 µs          | 🔴0.92     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 83.455 µs            | 98.501 µs          | 🔴**0.85** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 755.97 µs            | 436.97 µs          | 🟢1.73     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 695.81 µs            | 334.28 µs          | 🟢2.08     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3050 ms            | 2.0755 ms          | 🟢1.11     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.9725 ms            | 972.46 µs          | 🟢2.03     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 567.82 µs            | 612.60 µs          | 🔴0.93     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.7784 ms            | 1.1942 ms          | 🟢3.16     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6559 ms            | 2.3775 ms          | 🟢1.96     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7129 ms            | 935.27 µs          | 🟢2.9      |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 746.30 µs            | 746.26 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.1778 ms            | 2.6763 ms          | 🟢1.56     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0157 ms            | 1.0943 ms          | 🔴0.93     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.6441 ms            | 2.2657 ms          | 🟢2.49     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 9.9956 ms            | 7.6862 ms          | 🟢1.3      |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.5927 ms            | 1.6668 ms          | 🔴0.96     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.7403 ms            | 2.8220 ms          | 🔴0.97     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.5372 ms            | 1.6919 ms          | 🟢2.09     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.939 ms            | 9.4886 ms          | 🟢1.26     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.249 ms            | 7.1151 ms          | 🟢3.13     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.6259 ms            | 4.3247 ms          | 🟢1.76     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.7182 ms            | 2.9133 ms          | 🔴0.93     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.3419 ms            | 2.2701 ms          | 🟢**3.67** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.893 ms            | 4.5396 ms          | 🟢2.62     |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.656 ms            | 4.0781 ms          | 🟢3.1      |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.7361 ms            | 3.9382 ms          | 🔴0.95     |
| 15199017     | LONDON          | 866              | 30,028,395 | 8.9794 ms            | 3.2383 ms          | 🟢2.77     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0572 ms            | 1.0644 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5647 ms            | 1.6556 ms          | 🟢1.55     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.150 ms            | 4.6913 ms          | 🟢2.38     |
| 16146267     | MERGE           | 473              | 19,204,593 | 7.9553 ms            | 2.6641 ms          | 🟢2.99     |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.8673 ms            | 2.2415 ms          | 🟢2.17     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 12.902 ms            | 7.5194 ms          | 🟢1.72     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.592 ms            | 7.4704 ms          | 🟢1.95     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.078 ms            | 6.8270 ms          | 🟢1.48     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.0980 ms            | 1.3208 ms          | 🟢1.59     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 8.8476 ms            | 6.0547 ms          | 🟢1.46     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.193 ms            | 12.952 ms          | 🟢1.48     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.1187 ms            | 3.6579 ms          | 🟢2.22     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.1865 ms            | 1.0023 ms          | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.6144 ms            | 2.3838 ms          | 🟢1.94     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.4225 ms            | 5.3978 ms          | 🟢1.75     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.277 ms            | 6.6355 ms          | 🟢1.7      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.013 ms            | 6.5137 ms          | 🟢1.84     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 783.71 µs            | 557.41 µs          | 🟢1.41     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.9146 ms            | 3.7853 ms          | 🟢1.56     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.6240 ms            | 2.9427 ms          | 🟢3.27     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1539 ms            | 1.3698 ms          | 🟢1.57     |

- We are currently **~1.8 times faster than sequential execution** on average.
- The **max speed up is x3.67** for a large block with few dependencies.
- The **max slow down is x0.85** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
