# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution. We pick `snmalloc` as the global memory allocator for Gigagas blocks and `rpmalloc` for Ethereum blocks.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup    |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 121.04          | 38.683        | 🟢3.13     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 169.17          | 41.076        | 🟢4.12     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 393.05          | 41.603        | 🟢**9.45** |

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
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.391           | 0.396         | 🔴0.99     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.592           | 1.584         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.174           | 0.176         | 🔴0.99     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.078           | 0.095         | 🔴0.82     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.064           | 0.081         | 🔴**0.79** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.65            | 0.359         | 🟢1.81     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.538           | 0.313         | 🟢1.72     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.307           | 1.196         | 🟢1.09     |
| 5283152      | BYZANTIUM       | 150              | 7,988,261  | 1.537           | 0.452         | 🟢3.4      |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.276           | 0.618         | 🟢2.06     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.627           | 0.636         | 🔴0.99     |
| 6137495      | BYZANTIUM       | 60               | 7,994,690  | 0.727           | 0.389         | 🟢1.87     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.59            | 0.538         | 🟢1.1      |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.613           | 0.666         | 🟢**3.92** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.677           | 1.432         | 🟢1.87     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.855           | 0.718         | 🟢2.58     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.411           | 0.415         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.537           | 1.438         | 🟢1.76     |
| 10760440     | ISTANBUL        | 202              | 12,466,618 | 3.382           | 1.373         | 🟢2.46     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.611           | 5.953         | 🟢1.45     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.059           | 1.089         | 🔴0.97     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.847           | 1.108         | 🟢2.57     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.788           | 1.385         | 🟢2.01     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.992           | 1.055         | 🟢2.84     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.779           | 3.37          | 🟢1.42     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.373           | 1.421         | 🔴0.97     |
| 12459406     | BERLIN          | 201              | 14,994,849 | 4.597           | 2.28          | 🟢2.02     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.8             | 1.843         | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 1.997           | 0.894         | 🟢2.23     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.033           | 3.383         | 🟢1.78     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.764          | 3.823         | 🟢3.34     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.229           | 1.655         | 🟢3.16     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.926           | 2.925         | 🔴0.1      |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.673           | 1.276         | 🟢3.66     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.394           | 2.084         | 🟢3.07     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.025           | 2.385         | 🟢2.95     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.242           | 1.453         | 🟢2.23     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.331           | 1.612         | 🟢3.31     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.085           | 1.06          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.626           | 1.147         | 🟢1.42     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.939           | 1.876         | 🟢3.17     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.851           | 1.731         | 🟢2.8      |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.269           | 0.965         | 🟢2.35     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.593           | 3.021         | 🟢2.18     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.285          | 5.833         | 🟢1.76     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.922           | 2.437         | 🟢2.02     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.674           | 1.102         | 🟢1.52     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.215           | 2.133         | 🟢1.98     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.101          | 6.178         | 🟢1.96     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.526           | 1.417         | 🟢2.49     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.482           | 0.403         | 🟢1.2      |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.432           | 1.316         | 🟢1.85     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.422           | 2.216         | 🟢2        |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.811           | 5.898         | 🟢1.32     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.054           | 2.52          | 🟢2.01     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.429           | 0.3           | 🟢1.43     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.669           | 1.589         | 🟢1.68     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.553           | 1.272         | 🟢3.58     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.062           | 0.631         | 🟢1.68     |

- We are currently **~2 times faster than sequential execution** on average.
- The **max speed up is x3.92** for a large block with few dependencies.
- The **max slow down is x0.79** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
