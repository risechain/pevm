# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup    |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 130.58          | 55.694        | 🟢2.34     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 169.35          | 50.778        | 🟢3.34     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 391.39          | 44.309        | 🟢**8.83** |

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

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential (ms) | Parallel (ms) | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | --------------- | ------------- | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | 0.002           | 0.002         | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 0.032           | 0.031         | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.063           | 0.062         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.404           | 0.41          | 🔴0.99     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.565           | 1.563         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.178           | 0.183         | 🔴0.97     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.079           | 0.094         | 🔴0.84     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.065           | 0.084         | 🔴**0.78** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.657           | 0.4           | 🟢1.64     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.534           | 0.338         | 🟢1.58     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.311           | 1.21          | 🟢1.08     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.278           | 0.623         | 🟢2.05     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.622           | 0.634         | 🔴0.98     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.591           | 0.571         | 🟢1.04     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.594           | 0.698         | 🟢**3.72** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.602           | 1.427         | 🟢1.82     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.86            | 0.803         | 🟢2.32     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.41            | 0.41          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.525           | 1.406         | 🟢1.8      |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.957           | 6.395         | 🟢1.4      |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.067           | 1.097         | 🔴0.97     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.83            | 1.201         | 🟢2.36     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.738           | 1.482         | 🟢1.85     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.951           | 1.146         | 🟢2.58     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.744           | 3.406         | 🟢1.39     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.365           | 1.4           | 🔴0.98     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.797           | 1.832         | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 1.985           | 0.948         | 🟢2.09     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.062           | 3.455         | 🟢1.75     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.759          | 4.06          | 🟢3.14     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.21            | 4.187         | 🟢1.24     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.919           | 2.954         | 🔴0.99     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.674           | 1.602         | 🟢2.92     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.342           | 3.29          | 🟢1.93     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.006           | 3.949         | 🟢2.38     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.268           | 3.364         | 🔴0.97     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.327           | 2.684         | 🟢1.98     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.047           | 1.048         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.651           | 1.19          | 🟢1.39     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.982           | 3.441         | 🟢1.74     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.85            | 2.035         | 🟢2.38     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.25            | 0.931         | 🟢2.42     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.534           | 3.14          | 🟢2.08     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.681          | 6.434         | 🟢1.66     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.941           | 2.5           | 🟢1.98     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.665           | 1.1           | 🟢1.51     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.204           | 2.245         | 🟢1.87     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.27           | 6.703         | 🟢1.83     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.572           | 1.47          | 🟢2.43     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.477           | 0.406         | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.392           | 1.321         | 🟢1.81     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.429           | 2.31          | 🟢1.92     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.933           | 6.072         | 🟢1.31     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.107           | 2.613         | 🟢1.95     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.438           | 0.3           | 🟢1.46     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.677           | 1.651         | 🟢1.62     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.546           | 1.406         | 🟢3.23     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.062           | 0.637         | 🟢1.67     |

- We are currently **~1.75 times faster than sequential execution** on average.
- The **max speed up is x3.72** for a large block with few dependencies.
- The **max slow down is x0.78** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
