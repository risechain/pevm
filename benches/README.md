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
| Raw Transfers   | 47,620           | 1,000,020,000 | 119.60          | 76.585        | 🟢1.56     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 166.96          | 51.076        | 🟢3.27     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 387.35          | 43.403        | 🟢**8.92** |

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
| 930196       | FRONTIER        | 18               | 378,000    | 0.032           | 0.032         | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.064           | 0.065         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.408           | 0.407         | ⚪1        |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.59            | 1.593         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.18            | 0.178         | ⚪1        |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.079           | 0.101         | 🔴0.78     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.066           | 0.084         | 🔴0.78     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.667           | 0.412         | 🟢1.62     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.544           | 0.329         | 🟢1.65     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.329           | 1.208         | 🟢1.1      |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.287           | 0.646         | 🟢1.99     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.637           | 0.647         | 🔴0.98     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.598           | 0.554         | 🟢1.08     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.664           | 0.681         | 🟢**3.91** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.663           | 1.441         | 🟢1.85     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.898           | 0.796         | 🟢2.38     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.415           | 0.414         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.519           | 1.452         | 🟢1.74     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.789           | 6.048         | 🟢1.45     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.097           | 1.108         | 🔴0.99     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.831           | 1.168         | 🟢2.42     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.763           | 1.431         | 🟢1.93     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.983           | 1.118         | 🟢2.67     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.767           | 3.391         | 🟢1.41     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.418           | 1.43          | 🔴0.99     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.851           | 1.861         | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 2.025           | 0.943         | 🟢2.15     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.074           | 3.491         | 🟢1.74     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.838          | 3.899         | 🟢3.29     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.313           | 1.85          | 🟢2.87     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.043           | 2.989         | 🟢1.02     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.852           | 1.507         | 🟢3.22     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.39            | 2.265         | 🟢2.82     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.137           | 2.546         | 🟢2.8      |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.334           | 1.73          | 🟢1.93     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.448           | 1.801         | 🟢3.02     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.03            | 1.027         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.622           | 1.144         | 🟢1.42     |
| 15538827     | MERGE           | 823              | 29,981,465 | 6.063           | 2.089         | 🟢2.9      |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.875           | 1.877         | 🟢2.6      |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.254           | 0.98          | 🟢2.3      |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.557           | 3.07          | 🟢2.14     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.503          | 6.039         | 🟢1.74     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.957           | 2.478         | 🟢2        |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.655           | 1.082         | 🟢1.53     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.27            | 2.239         | 🟢1.91     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.158          | 6.404         | 🟢1.9      |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.565           | 1.445         | 🟢2.47     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.481           | 0.408         | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.427           | 1.326         | 🟢1.83     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.426           | 2.28          | 🟢1.94     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.918           | 6.044         | 🟢1.31     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.107           | 2.612         | 🟢1.96     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.436           | 0.308         | 🟢1.41     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.681           | 1.608         | 🟢1.67     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.55            | 1.325         | 🟢3.43     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.07            | 0.635         | 🟢1.69     |

- We are currently **~1.93 times faster than sequential execution** on average.
- The **max speed up is x3.91** for a large block with few dependencies.
- The **max slow down is x0.78** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
