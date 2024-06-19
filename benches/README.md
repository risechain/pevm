# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup   |
| --------------- | ---------------- | ------------- | --------------- | ------------- | --------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 121.45          | 56.419        | 🟢2.15    |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 170.31          | 49.567        | 🟢3.44    |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 391.69          | 42.592        | 🟢**9.2** |

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
| 930196       | FRONTIER        | 18               | 378,000    | 0.031           | 0.031         | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.063           | 0.063         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.391           | 0.402         | 🔴0.97     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.576           | 1.565         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.178           | 0.179         | 🔴0.99     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.077           | 0.093         | 🔴0.82     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.064           | 0.08          | 🔴**0.81** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.652           | 0.411         | 🟢1.59     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.548           | 0.319         | 🟢1.72     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.316           | 1.161         | 🟢1.13     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.268           | 0.596         | 🟢2.13     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.633           | 0.64          | 🔴0.99     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.598           | 0.528         | 🟢1.13     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.589           | 0.671         | 🟢**3.86** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.641           | 1.429         | 🟢1.85     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.852           | 0.768         | 🟢2.41     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.412           | 0.411         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.52            | 1.409         | 🟢1.79     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.966           | 6.381         | 🟢1.41     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.065           | 1.111         | 🔴0.96     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.823           | 1.176         | 🟢2.4      |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.783           | 1.444         | 🟢1.93     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.99            | 1.115         | 🟢2.68     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.783           | 3.374         | 🟢1.42     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.397           | 1.414         | 🔴0.99     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.808           | 1.84          | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 2               | 0.905         | 🟢2.21     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.141           | 3.447         | 🟢1.78     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.834          | 3.933         | 🟢3.26     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.215           | 3.982         | 🟢1.31     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.929           | 2.965         | 🔴0.99     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.701           | 1.5           | 🟢3.13     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.301           | 3.095         | 🟢2.04     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.005           | 2.805         | 🟢2.5      |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.206           | 3.324         | 🔴0.96     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.331           | 2.52          | 🟢2.12     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.07            | 1.069         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.661           | 1.168         | 🟢1.42     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.891           | 3.238         | 🟢1.82     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.844           | 1.973         | 🟢2.45     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.258           | 0.913         | 🟢2.47     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.587           | 3.04          | 🟢2.17     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.711          | 6.371         | 🟢1.68     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 5.032           | 2.437         | 🟢2.07     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.658           | 1.084         | 🟢1.53     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.248           | 2.231         | 🟢1.9      |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.279          | 6.596         | 🟢1.86     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.611           | 1.434         | 🟢2.52     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.481           | 0.404         | 🟢1.19     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.426           | 1.285         | 🟢1.89     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.529           | 2.247         | 🟢2.02     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 8.031           | 6.047         | 🟢1.33     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.134           | 2.551         | 🟢2.01     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.44            | 0.298         | 🟢1.48     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.684           | 1.607         | 🟢1.67     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.586           | 1.369         | 🟢3.35     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.076           | 0.632         | 🟢1.7      |

- We are currently **~1.8 times faster than sequential execution** on average.
- The **max speed up is x3.86** for a large block with few dependencies.
- The **max slow down is x0.81** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
