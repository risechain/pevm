# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution. We pick `rpmalloc` as the global memory allocator; it has beaten `jemalloc`, `mimalloc`, and `snmalloc` in these benchmarks.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup     |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ----------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 101.02          | 39.237        | 🟢2.57      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 145.74          | 34.388        | 🟢4.24      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 363.59          | 27.685        | 🟢**13.13** |

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
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.392           | 0.261         | 🟢1.5      |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.553           | 1.539         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.177           | 0.174         | ⚪1        |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.078           | 0.078         | ⚪1        |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.064           | 0.064         | ⚪1        |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.658           | 0.283         | 🟢2.32     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.542           | 0.303         | 🟢1.79     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.312           | 1.209         | 🟢1.09     |
| 4864590      | BYZANTIUM       | 195              | 7,985,890  | 1.558           | 0.457         | 🟢3.41     |
| 5283152      | BYZANTIUM       | 150              | 7,988,261  | 1.513           | 0.437         | 🟢3.46     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.263           | 0.593         | 🟢2.13     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.625           | 0.334         | 🟢1.87     |
| 6137495      | BYZANTIUM       | 60               | 7,994,690  | 0.724           | 0.379         | 🟢1.91     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.584           | 0.53          | 🟢1.1      |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.596           | 0.654         | 🟢**3.97** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.643           | 1.412         | 🟢1.87     |
| 8038679      | PETERSBURG      | 237              | 7,993,635  | 1.293           | 0.518         | 🟢2.5      |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.836           | 0.673         | 🟢2.73     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.413           | 0.415         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.526           | 1.431         | 🟢1.76     |
| 10760440     | ISTANBUL        | 202              | 12,466,618 | 3.321           | 1.388         | 🟢2.39     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.578           | 5.976         | 🟢1.44     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.078           | 0.588         | 🟢1.83     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.796           | 1.102         | 🟢2.54     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.747           | 1.464         | 🟢1.88     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.951           | 1.083         | 🟢2.72     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.758           | 3.329         | 🟢1.43     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.422           | 0.703         | 🟢2.02     |
| 12459406     | BERLIN          | 201              | 14,994,849 | 4.503           | 2.447         | 🟢1.84     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.823           | 1.076         | 🟢1.69     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 1.993           | 0.881         | 🟢2.26     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.082           | 3.344         | 🟢1.82     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.727          | 3.862         | 🟢3.3      |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.182           | 1.728         | 🟢3        |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.809           | 1.401         | 🟢2.0      |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.603           | 1.304         | 🟢3.53     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.239           | 2.093         | 🟢2.98     |
| 14383540     | LONDON          | 722              | 30,059,751 | 6.972           | 2.444         | 🟢2.85     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.136           | 1.623         | 🟢1.93     |
| 14545870     | LONDON          | 456              | 29,925,884 | 8.031           | 2.564         | 🟢3.13     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.314           | 1.668         | 🟢3.18     |
| 15274915     | LONDON          | 1226             | 29,928,443 | 4.018           | 1.622         | 🟢2.48     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.039           | 1.039         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.577           | 1.099         | 🟢1.43     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.897           | 1.943         | 🟢3.04     |
| 15752489     | MERGE           | 132              | 8,242,594  | 1.808           | 0.828         | 🟢2.18     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.817           | 1.741         | 🟢2.77     |
| 16257471     | MERGE           | 98               | 20,267,875 | 6.579           | 4.263         | 🟢1.54     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.27            | 0.955         | 🟢2.38     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.599           | 2.907         | 🟢2.27     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.384          | 5.941         | 🟢1.75     |
| 18085863     | SHANGHAI        | 178              | 17,007,666 | 4.898           | 2.988         | 🟢1.64     |
| 18426253     | SHANGHAI        | 147              | 18,889,343 | 7.383           | 5.312         | 🟢1.39     |
| 18988207     | SHANGHAI        | 186              | 12,398,324 | 7.676           | 5.105         | 🟢1.5      |
| 19426586     | SHANGHAI        | 127              | 15,757,891 | 4.919           | 2.472         | 🟢1.99     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.667           | 1.088         | 🟢1.53     |
| 19498855     | CANCUN          | 241              | 29,919,049 | 10.373          | 5.417         | 🟢1.91     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.208           | 2.129         | 🟢1.98     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 11.978          | 6.217         | 🟢1.93     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.554           | 1.512         | 🟢2.35     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.485           | 0.481         | ⚪1        |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.418           | 1.286         | 🟢1.88     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.422           | 2.268         | 🟢1.95     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.735           | 5.968         | 🟢1.3      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.152           | 2.515         | 🟢2.05     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.432           | 0.279         | 🟢1.55     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.693           | 1.586         | 🟢1.7      |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.543           | 1.305         | 🟢3.48     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.077           | 0.618         | 🟢1.74     |

- We are currently **~2 times faster than sequential execution** on average.
- The **max speed up is x3.97** for a large block with few dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
