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
| Raw Transfers   | 47,620           | 1,000,020,000 | 121.80          | 37.478        | 🟢3.25     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 169.34          | 39.961        | 🟢4.24     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 387.92          | 41.067        | 🟢**9.45** |

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
| 930196       | FRONTIER        | 18               | 378,000    | 0.031           | 0.03          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.063           | 0.064         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.39            | 0.4           | ⚪1        |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.567           | 1.584         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.172           | 0.177         | 🔴0.97     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.077           | 0.097         | 🔴**0.8**  |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.066           | 0.081         | 🔴0.82     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.645           | 0.34          | 🟢1.9      |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.54            | 0.315         | 🟢1.71     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.321           | 1.192         | 🟢1.11     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.279           | 0.614         | 🟢2.08     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.609           | 0.632         | 🔴0.96     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.587           | 0.549         | 🟢1.07     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.615           | 0.66          | 🟢**3.96** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.638           | 1.424         | 🟢1.85     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.851           | 0.717         | 🟢2.58     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.413           | 0.415         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.535           | 1.451         | 🟢1.75     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.601           | 5.92          | 🟢1.45     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.05            | 1.086         | 🔴0.97     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.825           | 1.128         | 🟢2.51     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.799           | 1.413         | 🟢1.98     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.985           | 1.061         | 🟢2.81     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.801           | 3.368         | 🟢1.43     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.368           | 1.429         | 🔴0.96     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.804           | 1.844         | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 2.014           | 0.891         | 🟢2.26     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.124           | 3.421         | 🟢1.79     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.98           | 3.832         | 🟢3.39     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.19            | 1.666         | 🟢3.12     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.884           | 2.938         | 🔴0.98     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.71            | 1.274         | 🟢3.7      |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.316           | 2.113         | 🟢2.99     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.153           | 2.412         | 🟢2.97     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.22            | 1.474         | 🟢2.18     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.362           | 1.686         | 🟢3.18     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.054           | 1.055         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.616           | 1.128         | 🟢1.43     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.902           | 1.901         | 🟢3.11     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.893           | 1.77          | 🟢2.76     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.267           | 0.959         | 🟢2.36     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.591           | 3.025         | 🟢2.18     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.247          | 5.841         | 🟢1.75     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.964           | 2.451         | 🟢2.03     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.656           | 1.094         | 🟢1.51     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.219           | 2.163         | 🟢1.95     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.118          | 6.25          | 🟢1.94     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.569           | 1.453         | 🟢2.46     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.482           | 0.402         | 🟢1.2      |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.461           | 1.309         | 🟢1.88     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.475           | 2.283         | 🟢1.96     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.751           | 5.925         | 🟢1.31     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.12            | 2.571         | 🟢1.99     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.433           | 0.302         | 🟢1.43     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.681           | 1.605         | 🟢1.67     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.571           | 1.286         | 🟢3.55     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.074           | 0.632         | 🟢1.7      |

- We are currently **~1.97 times faster than sequential execution** on average.
- The **max speed up is x3.96** for a large block with few dependencies.
- The **max slow down is x0.8** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
