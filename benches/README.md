# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | Speedup    |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 126.97 ms            | 49.932 ms          | 🟢2.54     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 207.54 ms            | 60.236 ms          | 🟢3.45     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 577.37 ms            | 58.175 ms          | 🟢**9.92** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.2617 µs            | 2.2409 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 31.390 µs            | 31.012 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 73.527 µs            | 73.923 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 410.03 µs            | 428.41 µs          | 🔴0.96     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6138 ms            | 1.6107 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 182.50 µs            | 188.41 µs          | 🔴0.97     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 107.76 µs            | 103.80 µs          | 🟢1.04     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 88.557 µs            | 89.367 µs          | 🔴0.99     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 825.26 µs            | 427.50 µs          | 🟢1.93     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 710.43 µs            | 346.90 µs          | 🟢2.05     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.4114 ms            | 2.3333 ms          | 🟢1.03     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.0676 ms            | 854.86 µs          | 🟢2.42     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 619.57 µs            | 637.40 µs          | 🔴0.97     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 849.41 µs            | 686.10 µs          | 🟢1.24     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.8427 ms            | 1.0772 ms          | 🟢3.57     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.7394 ms            | 2.2969 ms          | 🟢2.06     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.8353 ms            | 939.57 µs          | 🟢3.02     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 761.56 µs            | 762.24 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.3163 ms            | 2.8202 ms          | 🟢1.53     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.1035 ms            | 1.1274 ms          | 🔴0.98     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.7809 ms            | 2.0288 ms          | 🟢2.85     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.420 ms            | 7.5438 ms          | 🟢1.38     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.6957 ms            | 1.7374 ms          | 🔴0.98     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.9342 ms            | 2.9712 ms          | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.6405 ms            | 1.5838 ms          | 🟢2.3      |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.357 ms            | 7.9066 ms          | 🟢1.56     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.772 ms            | 6.9968 ms          | 🟢3.25     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.9822 ms            | 4.3146 ms          | 🟢1.85     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.0888 ms            | 3.0979 ms          | 🔴1        |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.6842 ms            | 2.1973 ms          | 🟢**3.95** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.221 ms            | 4.6335 ms          | 🟢2.64     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.072 ms            | 4.0955 ms          | 🟢3.19     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.0473 ms            | 4.1420 ms          | 🔴0.98     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.3571 ms            | 3.2059 ms          | 🟢2.92     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0729 ms            | 1.0685 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.6359 ms            | 1.5789 ms          | 🟢1.67     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.763 ms            | 4.5705 ms          | 🟢2.57     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.2243 ms            | 2.5050 ms          | 🟢3.28     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.0091 ms            | 1.9908 ms          | 🟢2.52     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.392 ms            | 6.3677 ms          | 🟢2.1      |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.084 ms            | 6.8892 ms          | 🟢2.19     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.489 ms            | 5.6572 ms          | 🟢1.85     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1243 ms            | 1.1700 ms          | 🟢1.82     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.2229 ms            | 5.1145 ms          | 🟢1.8      |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.782 ms            | 7.6415 ms          | 🟢2.59     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.3242 ms            | 3.5249 ms          | 🟢2.36     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2363 ms            | 941.06 µs          | 🟢1.31     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.7582 ms            | 2.3383 ms          | 🟢2.03     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.8105 ms            | 5.0517 ms          | 🟢1.94     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.635 ms            | 6.5583 ms          | 🟢1.77     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.466 ms            | 5.9603 ms          | 🟢2.09     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 817.81 µs            | 496.40 µs          | 🟢1.65     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1133 ms            | 3.3365 ms          | 🟢1.83     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.9923 ms            | 2.8904 ms          | 🟢3.46     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.2508 ms            | 1.3056 ms          | 🟢1.72     |

- We are currently **~2.08 times faster than sequential execution** on average.
- The **max speed up is x3.95** for a large block with few dependencies.
- The **max slow down is x0.96** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
