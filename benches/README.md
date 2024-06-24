# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup    |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 123.84          | 38.464        | 🟢3.22     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 167.76          | 32.077        | 🟢5.23     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 382.22          | 40.706        | 🟢**9.39** |

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
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.391           | 0.398         | 🔴0.98     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.565           | 1.564         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.176           | 0.177         | 🔴1        |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.077           | 0.098         | 🔴0.79     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.064           | 0.083         | 🔴**0.78** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.645           | 0.328         | 🟢1.97     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.533           | 0.311         | 🟢1.72     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.308           | 1.19          | 🟢1.1      |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.27            | 0.606         | 🟢2.1      |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.619           | 0.64          | 🔴0.97     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.586           | 0.53          | 🟢1.11     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.598           | 0.66          | 🟢**3.93** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.636           | 1.422         | 🟢1.85     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.87            | 0.705         | 🟢2.65     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.413           | 0.412         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.504           | 1.432         | 🟢1.75     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.574           | 5.957         | 🟢1.44     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.066           | 1.104         | 🔴0.97     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.792           | 1.106         | 🟢2.52     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.731           | 1.389         | 🟢1.97     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.957           | 1.047         | 🟢2.82     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.723           | 3.334         | 🟢1.42     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.387           | 1.397         | 🔴0.99     |
| 12459406     | BERLIN          | 201              | 14,994,849 | 4.58            | 2.272         | 🟢2.02     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.807           | 1.853         | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 1.988           | 0.89          | 🟢2.23     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.005           | 3.442         | 🟢1.74     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.724          | 3.841         | 🟢3.31     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.198           | 1.624         | 🟢3.2      |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.904           | 2.987         | 🔴0.97     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.631           | 1.258         | 🟢3.68     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.287           | 2.067         | 🟢3.04     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.112           | 2.34          | 🟢3.04     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.229           | 1.368         | 🟢2.36     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.319           | 1.578         | 🟢3.37     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.056           | 1.058         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.636           | 1.157         | 🟢1.41     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.858           | 1.845         | 🟢3.17     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.837           | 1.779         | 🟢2.72     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.242           | 0.969         | 🟢2.31     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.518           | 2.992         | 🟢2.18     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.291          | 5.853         | 🟢1.76     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.943           | 2.419         | 🟢2.04     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.659           | 1.096         | 🟢1.51     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.186           | 2.108         | 🟢1.99     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 11.992          | 6.218         | 🟢1.93     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.562           | 1.416         | 🟢2.51     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.475           | 0.404         | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.401           | 1.318         | 🟢1.82     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.43            | 2.24          | 🟢1.98     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.749           | 5.921         | 🟢1.31     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.054           | 2.517         | 🟢2.01     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.431           | 0.298         | 🟢1.45     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.652           | 1.57          | 🟢1.69     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.512           | 1.258         | 🟢3.59     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.061           | 0.624         | 🟢1.7      |

- We are currently **~1.98 times faster than sequential execution** on average.
- The **max speed up is x3.93** for a large block with few dependencies.
- The **max slow down is x0.78** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
