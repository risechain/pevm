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
| Raw Transfers   | 47,620           | 1,000,020,000 | 123.11          | 38.830        | 🟢3.17     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 170.71          | 32.171        | 🟢5.31     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 384.33          | 41.451        | 🟢**9.27** |

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
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.393           | 0.402         | 🔴0.98     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.567           | 1.566         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.176           | 0.179         | 🔴0.99     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.077           | 0.098         | 🔴**0.78** |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.065           | 0.082         | 🔴0.79     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.653           | 0.332         | 🟢1.97     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.546           | 0.316         | 🟢1.73     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.316           | 1.198         | 🟢1.11     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.275           | 0.61          | 🟢2.09     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.641           | 0.651         | 🔴0.99     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.597           | 0.529         | 🟢1.13     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.606           | 0.663         | 🟢**3.93** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.643           | 1.43          | 🟢1.85     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.876           | 0.707         | 🟢2.65     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.418           | 0.418         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.523           | 1.437         | 🟢1.76     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 8.533           | 5.955         | 🟢1.43     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.094           | 1.105         | 🔴0.99     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 2.801           | 1.105         | 🟢2.53     |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 2.736           | 1.381         | 🟢1.98     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.997           | 1.05          | 🟢2.86     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.728           | 3.362         | 🟢1.41     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.405           | 1.423         | 🔴0.99     |
| 12459406     | BERLIN          | 201              | 14,994,849 | 4.561           | 2.278         | 🟢2        |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.842           | 1.856         | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 1.978           | 0.884         | 🟢2.24     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.011           | 3.418         | 🟢1.76     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.736          | 3.818         | 🟢3.34     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.244           | 1.639         | 🟢3.2      |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.961           | 2.984         | 🔴0.99     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.652           | 1.282         | 🟢3.63     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.274           | 2.081         | 🟢3.01     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.091           | 2.391         | 🟢2.97     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.263           | 1.431         | 🟢2.28     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.426           | 1.6           | 🟢3.39     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.047           | 1.054         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.628           | 1.163         | 🟢1.4      |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.917           | 1.865         | 🟢3.17     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.841           | 1.748         | 🟢2.77     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.245           | 0.969         | 🟢2.32     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.529           | 2.985         | 🟢2.19     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.295          | 5.86          | 🟢1.76     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 4.906           | 2.449         | 🟢2        |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.647           | 1.096         | 🟢1.5      |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.189           | 2.128         | 🟢1.97     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.067          | 6.213         | 🟢1.94     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.534           | 1.418         | 🟢2.49     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.474           | 0.404         | 🟢1.17     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.408           | 1.316         | 🟢1.83     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.387           | 2.218         | 🟢1.98     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.731           | 5.935         | 🟢1.3      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.123           | 2.506         | 🟢2.04     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.428           | 0.298         | 🟢1.44     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.667           | 1.571         | 🟢1.7      |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.518           | 1.267         | 🟢3.57     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.073           | 0.634         | 🟢1.69     |

- We are currently **~1.98 times faster than sequential execution** on average.
- The **max speed up is x3.93** for a large block with few dependencies.
- The **max slow down is x0.78** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
