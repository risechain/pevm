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
| Raw Transfers   | 47,620           | 1,000,020,000 | 127.12 ms            | 85.390 ms          | 🟢1.49     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 219.21 ms            | 70.958 ms          | 🟢3.09     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 571.47 ms            | 58.167 ms          | 🟢**9.82** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.0146 µs            | 2.0542 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 26.144 µs            | 26.310 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 69.237 µs            | 69.015 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 361.75 µs            | 396.76 µs          | 🔴0.91     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.4224 ms            | 1.4242 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 164.30 µs            | 175.19 µs          | 🔴0.94     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 99.253 µs            | 110.03 µs          | 🔴0.9      |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 81.810 µs            | 96.557 µs          | 🔴**0.85** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 758.26 µs            | 407.19 µs          | 🟢1.86     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 698.69 µs            | 331.56 µs          | 🟢2.11     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3354 ms            | 2.1221 ms          | 🟢1.1      |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 565.96 µs            | 609.89 µs          | 🔴0.93     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.7921 ms            | 1.2101 ms          | 🟢3.13     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6770 ms            | 2.4034 ms          | 🟢1.95     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7315 ms            | 925.54 µs          | 🟢2.95     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 752.06 µs            | 756.43 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.2872 ms            | 2.7849 ms          | 🟢1.54     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0186 ms            | 1.0930 ms          | 🔴0.93     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.7246 ms            | 2.8274 ms          | 🟢2.02     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.305 ms            | 7.9624 ms          | 🟢1.29     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.6164 ms            | 1.7072 ms          | 🔴0.95     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8276 ms            | 2.9042 ms          | 🔴0.97     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.5718 ms            | 1.7736 ms          | 🟢2.01     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.167 ms            | 9.7853 ms          | 🟢1.24     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.563 ms            | 7.5589 ms          | 🟢2.98     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.6696 ms            | 4.4923 ms          | 🟢1.71     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.7321 ms            | 2.9145 ms          | 🔴0.94     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.5445 ms            | 2.3487 ms          | 🟢**3.64** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.084 ms            | 7.2938 ms          | 🟢1.66     |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.913 ms            | 5.4041 ms          | 🟢2.39     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.8064 ms            | 3.9867 ms          | 🔴0.95     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.0909 ms            | 3.2360 ms          | 🟢2.81     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0717 ms            | 1.0677 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.6255 ms            | 1.7527 ms          | 🟢1.5      |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.536 ms            | 5.6157 ms          | 🟢2.05     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.0752 ms            | 2.6875 ms          | 🟢3        |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.9892 ms            | 2.4351 ms          | 🟢2.05     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.304 ms            | 8.6351 ms          | 🟢1.54     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.824 ms            | 8.0059 ms          | 🟢1.85     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.413 ms            | 8.4760 ms          | 🟢1.23     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1070 ms            | 1.3122 ms          | 🟢1.61     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.1300 ms            | 6.1596 ms          | 🟢1.48     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.563 ms            | 8.0585 ms          | 🟢2.43     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.2538 ms            | 4.2188 ms          | 🟢1.96     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2262 ms            | 1.0448 ms          | 🟢1.17     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.7410 ms            | 2.6935 ms          | 🟢1.76     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.7150 ms            | 5.7822 ms          | 🟢1.68     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.450 ms            | 6.6407 ms          | 🟢1.72     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.344 ms            | 7.5275 ms          | 🟢1.64     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 802.51 µs            | 572.67 µs          | 🟢1.4      |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.0550 ms            | 4.7224 ms          | 🟢1.28     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.9202 ms            | 3.8320 ms          | 🟢2.59     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.2496 ms            | 1.4396 ms          | 🟢1.56     |

- We are currently **~1.8 times faster than sequential execution** on average.
- The **max speed up is x3.64** for a large block with few dependencies.
- The **max slow down is x0.85** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
