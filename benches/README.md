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
| Raw Transfers   | 47,620           | 1,000,020,000 | 126.01 ms            | 52.370 ms          | 🟢2.41      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 207.01 ms            | 66.209 ms          | 🟢3.13      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 623.01 ms            | 60.655 ms          | 🟢**10.27** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.2594 µs            | 2.2432 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 30.979 µs            | 30.753 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 72.804 µs            | 72.736 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 406.51 µs            | 427.34 µs          | 🔴0.95     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6019 ms            | 1.6181 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 180.80 µs            | 190.46 µs          | 🔴0.95     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 106.15 µs            | 105.04 µs          | 🟢1.01     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.411 µs            | 90.295 µs          | 🔴0.97     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 807.42 µs            | 410.34 µs          | 🟢1.97     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 704.80 µs            | 349.10 µs          | 🟢2.02     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3140 ms            | 2.2246 ms          | 🟢1.04     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.0204 ms            | 837.40 µs          | 🟢2.41     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 606.85 µs            | 654.34 µs          | 🔴**0.93** |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.7863 ms            | 1.0557 ms          | 🟢3.59     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6491 ms            | 2.2339 ms          | 🟢2.08     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7536 ms            | 932.96 µs          | 🟢2.95     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 746.90 µs            | 757.35 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.2143 ms            | 2.7092 ms          | 🟢1.56     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0781 ms            | 1.1329 ms          | 🔴0.95     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.7014 ms            | 1.9877 ms          | 🟢2.87     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 9.9708 ms            | 7.1997 ms          | 🟢1.38     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.7146 ms            | 1.7471 ms          | 🔴0.98     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8293 ms            | 2.8823 ms          | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.5506 ms            | 1.5597 ms          | 🟢2.28     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.986 ms            | 7.6329 ms          | 🟢1.57     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.268 ms            | 6.7609 ms          | 🟢3.29     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.7800 ms            | 4.2503 ms          | 🟢1.83     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.0484 ms            | 3.1302 ms          | 🔴0.97     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.5631 ms            | 2.1875 ms          | 🟢**3.91** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.861 ms            | 4.5272 ms          | 🟢2.62     |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.784 ms            | 4.0075 ms          | 🟢3.19     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.0509 ms            | 4.1201 ms          | 🔴0.98     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.0523 ms            | 3.1843 ms          | 🟢2.84     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0708 ms            | 1.0654 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5430 ms            | 1.5036 ms          | 🟢1.69     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.416 ms            | 4.4836 ms          | 🟢2.55     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.0458 ms            | 2.4961 ms          | 🟢3.22     |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.9050 ms            | 1.8947 ms          | 🟢2.59     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 12.971 ms            | 6.0755 ms          | 🟢2.13     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.720 ms            | 6.8843 ms          | 🟢2.14     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.318 ms            | 5.4642 ms          | 🟢1.89     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1003 ms            | 1.1589 ms          | 🟢1.81     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 8.9353 ms            | 4.8263 ms          | 🟢1.85     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.275 ms            | 7.4181 ms          | 🟢2.6      |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.0559 ms            | 3.3626 ms          | 🟢2.4      |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2006 ms            | 899.93 µs          | 🟢1.33     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.6460 ms            | 2.2484 ms          | 🟢2.07     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.4386 ms            | 4.8319 ms          | 🟢1.95     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.338 ms            | 6.5480 ms          | 🟢1.73     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.099 ms            | 5.6906 ms          | 🟢2.13     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 799.21 µs            | 488.35 µs          | 🟢1.64     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.9440 ms            | 3.2220 ms          | 🟢1.84     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.6365 ms            | 2.8021 ms          | 🟢3.44     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1835 ms            | 1.2420 ms          | 🟢1.76     |

- We are currently **~2.08 times faster than sequential execution** on average.
- The **max speed up is x3.91** for a large block with few dependencies.
- The **max slow down is x0.93** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
