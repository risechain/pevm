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
| Raw Transfers   | 47,620           | 1,000,020,000 | 128.75 ms            | 85.599 ms          | 🟢1.5       |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 224.16 ms            | 71.718 ms          | 🟢3.13      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 623.64 ms            | 59.867 ms          | 🟢**10.42** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.1678 µs            | 2.1861 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 28.423 µs            | 28.446 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 72.579 µs            | 71.040 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 381.13 µs            | 413.16 µs          | 🔴0.92     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.4926 ms            | 1.4957 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 171.53 µs            | 182.73 µs          | 🔴0.94     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 102.69 µs            | 112.15 µs          | 🔴0.92     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 83.568 µs            | 101.37 µs          | 🔴**0.82** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 784.34 µs            | 418.51 µs          | 🟢1.87     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 704.88 µs            | 338.82 µs          | 🟢2.08     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3780 ms            | 2.1436 ms          | 🟢1.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 604.63 µs            | 644.76 µs          | 🔴0.94     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.8354 ms            | 1.2198 ms          | 🟢3.14     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6129 ms            | 2.4186 ms          | 🟢1.91     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7887 ms            | 952.19 µs          | 🟢2.93     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 761.44 µs            | 765.58 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.3566 ms            | 2.8030 ms          | 🟢1.55     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0643 ms            | 1.1542 ms          | 🔴0.92     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.7084 ms            | 2.8479 ms          | 🟢2        |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.363 ms            | 7.9983 ms          | 🟢1.3      |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.7061 ms            | 1.7901 ms          | 🔴0.95     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.9218 ms            | 2.9879 ms          | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.6240 ms            | 1.7884 ms          | 🟢2.03     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.968 ms            | 9.7646 ms          | 🟢1.23     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.799 ms            | 7.6591 ms          | 🟢2.98     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.7835 ms            | 4.6295 ms          | 🟢1.68     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.8604 ms            | 3.0877 ms          | 🔴0.93     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.5388 ms            | 2.3301 ms          | 🟢**3.66** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.998 ms            | 6.2277 ms          | 🟢1.93     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.081 ms            | 5.4617 ms          | 🟢2.4      |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.9488 ms            | 4.1612 ms          | 🔴0.95     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.2107 ms            | 3.3184 ms          | 🟢2.78     |
| 15537393     | LONDON          | 1                | 29,991,429 | 9.7170 µs            | 9.7763 µs          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.6215 ms            | 1.7571 ms          | 🟢1.49     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.668 ms            | 5.7470 ms          | 🟢2.03     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.2149 ms            | 2.7287 ms          | 🟢3.01     |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.9936 ms            | 2.4445 ms          | 🟢2.04     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.504 ms            | 8.6991 ms          | 🟢1.55     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.107 ms            | 8.0470 ms          | 🟢1.88     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.515 ms            | 8.6287 ms          | 🟢1.22     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1246 ms            | 1.3126 ms          | 🟢1.62     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.1882 ms            | 6.3763 ms          | 🟢1.44     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.785 ms            | 7.8779 ms          | 🟢2.51     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.3749 ms            | 4.2406 ms          | 🟢1.97     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2434 ms            | 1.0533 ms          | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.7662 ms            | 2.7093 ms          | 🟢1.76     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.7693 ms            | 5.8017 ms          | 🟢1.68     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.546 ms            | 6.6474 ms          | 🟢1.74     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.454 ms            | 7.5972 ms          | 🟢1.64     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 820.96 µs            | 579.98 µs          | 🟢1.42     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.9319 ms            | 3.7168 ms          | 🟢1.6      |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.072 ms            | 3.8285 ms          | 🟢2.63     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.2437 ms            | 1.4456 ms          | 🟢1.55     |

- We are currently **~1.8 times faster than sequential execution** on average.
- The **max speed up is x3.66** for a large block with few dependencies.
- The **max slow down is x0.82** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
