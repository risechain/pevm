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
| Raw Transfers   | 47,620           | 1,000,020,000 | 134.14 ms            | 86.042 ms          | 🟢1.56      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 226.39 ms            | 71.340 ms          | 🟢3.17      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 590.58 ms            | 58.886 ms          | 🟢**10.03** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.9108 µs            | 2.9294 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 39.073 µs            | 38.779 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 82.120 µs            | 105.29 µs          | 🔴0.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 493.08 µs            | 556.83 µs          | 🔴0.89     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.7944 ms            | 1.7516 ms          | 🟢1.02     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 222.52 µs            | 366.60 µs          | 🔴**0.61** |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 120.48 µs            | 105.82 µs          | 🟢1.14     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 98.244 µs            | 100.22 µs          | 🔴0.98     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 950.52 µs            | 484.96 µs          | 🟢1.96     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 775.48 µs            | 361.38 µs          | 🟢2.15     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.5568 ms            | 2.2631 ms          | 🟢1.13     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 780.81 µs            | 849.59 µs          | 🔴0.92     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.0308 ms            | 1.2823 ms          | 🟢3.14     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8439 ms            | 2.4765 ms          | 🟢1.96     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.0742 ms            | 1.1678 ms          | 🟢2.63     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 789.72 µs            | 939.62 µs          | 🔴0.84     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.3955 ms            | 2.7950 ms          | 🟢1.57     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.3414 ms            | 1.4711 ms          | 🔴0.91     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.0226 ms            | 2.9906 ms          | 🟢2.01     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.509 ms            | 7.9987 ms          | 🟢1.31     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 2.0348 ms            | 2.1543 ms          | 🔴0.94     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.2372 ms            | 3.3638 ms          | 🔴0.96     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.8477 ms            | 1.9073 ms          | 🟢2.02     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.222 ms            | 9.8242 ms          | 🟢1.24     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.488 ms            | 7.9074 ms          | 🟢2.97     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.3157 ms            | 5.2064 ms          | 🟢1.6      |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.4159 ms            | 3.7051 ms          | 🔴0.92     |
| 14029313     | LONDON          | 724              | 30,074,554 | 9.1521 ms            | 2.5977 ms          | 🟢**3.52** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.616 ms            | 6.2625 ms          | 🟢2.01     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.729 ms            | 5.6561 ms          | 🟢2.43     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.4545 ms            | 4.7803 ms          | 🔴0.93     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.8210 ms            | 3.6872 ms          | 🟢2.66     |
| 15537393     | LONDON          | 1                | 29,991,429 | 10.691 µs            | 10.572 µs          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9041 ms            | 1.8463 ms          | 🟢1.57     |
| 15538827     | MERGE           | 823              | 29,981,465 | 12.093 ms            | 6.0053 ms          | 🟢2.01     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.5698 ms            | 2.9293 ms          | 🟢2.93     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1256 ms            | 2.4599 ms          | 🟢2.08     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.739 ms            | 8.7428 ms          | 🟢1.57     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.664 ms            | 8.0454 ms          | 🟢1.95     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.754 ms            | 8.5403 ms          | 🟢1.26     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1885 ms            | 1.3489 ms          | 🟢1.62     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.4719 ms            | 6.4102 ms          | 🟢1.48     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 20.340 ms            | 8.4852 ms          | 🟢2.4      |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.4804 ms            | 4.2449 ms          | 🟢2        |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2728 ms            | 1.0424 ms          | 🟢1.22     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.9268 ms            | 2.7874 ms          | 🟢1.77     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.000 ms            | 5.8780 ms          | 🟢1.7      |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.922 ms            | 6.8196 ms          | 🟢1.75     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.653 ms            | 7.5437 ms          | 🟢1.68     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 870.94 µs            | 605.58 µs          | 🟢1.44     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.0423 ms            | 3.7374 ms          | 🟢1.62     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.133 ms            | 3.8500 ms          | 🟢2.63     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.2518 ms            | 1.3862 ms          | 🟢1.62     |

- We are currently **~1.8 times faster than sequential execution** on average.
- The **max speed up is x3.52** for a large block with few dependencies.
- The **max slow down is x0.61** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
