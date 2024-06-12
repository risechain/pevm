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
| Raw Transfers   | 47,620           | 1,000,020,000 | 125.85 ms            | 49.904 ms          | 🟢2.52      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 205.67 ms            | 60.366 ms          | 🟢3.4       |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 635.36 ms            | 59.858 ms          | 🟢**10.61** |

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

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | Speedup   |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | --------- |
| 46147        | FRONTIER        | 1                | 21,000     | 2.2741 µs            | 2.2765 µs          | ⚪1       |
| 930196       | FRONTIER        | 18               | 378,000    | 31.468 µs            | 31.458 µs          | ⚪1       |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 73.585 µs            | 74.045 µs          | ⚪1       |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 418.36 µs            | 430.82 µs          | 🔴0.97    |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6223 ms            | 1.6300 ms          | ⚪1       |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 183.93 µs            | 190.05 µs          | 🔴0.97    |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 106.56 µs            | 105.16 µs          | 🟢1.01    |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.226 µs            | 91.270 µs          | 🔴0.96    |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 814.47 µs            | 398.08 µs          | 🟢2.05    |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 717.46 µs            | 349.52 µs          | 🟢2.05    |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3429 ms            | 2.2352 ms          | 🟢1.05    |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.0185 ms            | 841.58 µs          | 🟢2.4     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 608.64 µs            | 635.83 µs          | 🔴0.96    |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.8107 ms            | 1.0535 ms          | 🟢3.62    |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6617 ms            | 2.2169 ms          | 🟢2.1     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7994 ms            | 930.07 µs          | 🟢3.01    |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 762.56 µs            | 764.82 µs          | ⚪1       |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.2259 ms            | 2.7126 ms          | 🟢1.56    |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0879 ms            | 1.1173 ms          | 🔴0.97    |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.6554 ms            | 1.9708 ms          | 🟢2.87    |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.027 ms            | 7.2210 ms          | 🟢1.39    |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.7190 ms            | 1.7405 ms          | 🔴0.99    |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8283 ms            | 2.8615 ms          | 🔴0.99    |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.5889 ms            | 1.5551 ms          | 🟢2.31    |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.010 ms            | 7.6562 ms          | 🟢1.57    |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.524 ms            | 6.7729 ms          | 🟢3.33    |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.8432 ms            | 4.2722 ms          | 🟢1.84    |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.0151 ms            | 3.1306 ms          | 🔴0.96    |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.6218 ms            | 2.2080 ms          | 🟢**3.9** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.936 ms            | 4.5247 ms          | 🟢2.64    |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.765 ms            | 3.9937 ms          | 🟢3.2     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.0285 ms            | 4.1414 ms          | 🔴0.97    |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.1545 ms            | 3.1697 ms          | 🟢2.89    |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0712 ms            | 1.0655 ms          | ⚪1       |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5530 ms            | 1.4970 ms          | 🟢1.71    |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.392 ms            | 4.4702 ms          | 🟢2.55    |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.0692 ms            | 2.4957 ms          | 🟢3.23    |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.8927 ms            | 1.9068 ms          | 🟢2.57    |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.147 ms            | 6.0670 ms          | 🟢2.17    |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.871 ms            | 6.8864 ms          | 🟢2.16    |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.179 ms            | 5.4659 ms          | 🟢1.86    |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1329 ms            | 1.1693 ms          | 🟢1.82    |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.1388 ms            | 4.7922 ms          | 🟢1.91    |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.411 ms            | 7.4359 ms          | 🟢2.61    |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.0686 ms            | 3.3837 ms          | 🟢2.38    |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.1937 ms            | 896.35 µs          | 🟢1.33    |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.6652 ms            | 2.2433 ms          | 🟢2.08    |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.5509 ms            | 4.8586 ms          | 🟢1.97    |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.434 ms            | 6.5601 ms          | 🟢1.74    |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.169 ms            | 5.6983 ms          | 🟢2.14    |
| 19933122     | CANCUN          | 45               | 2,056,821  | 811.87 µs            | 487.19 µs          | 🟢1.67    |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.9492 ms            | 3.2328 ms          | 🟢1.84    |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.6613 ms            | 2.7897 ms          | 🟢3.46    |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1515 ms            | 1.2422 ms          | 🟢1.73    |

- We are currently **~2.09 times faster than sequential execution** on average.
- The **max speed up is x3.9** for a large block with few dependencies.
- The **max slow down is x0.96** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
