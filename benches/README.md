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
| Raw Transfers   | 47,620           | 1,000,020,000 | 128.11          | 55.756        | 🟢2.3      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 168.39          | 50.054        | 🟢3.36     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 390.86          | 44.263        | 🟢**8.83** |

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
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.064           | 0.064         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.389           | 0.397         | 🔴0.98     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.589           | 1.6           | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.172           | 0.179         | 🔴0.96     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.078           | 0.096         | 🔴0.82     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.066           | 0.084         | 🔴**0.78** |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.65            | 0.406         | 🟢1.6      |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.541           | 0.338         | 🟢1.6      |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 1.35            | 1.205         | 🟢1.12     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.3             | 0.616         | 🟢2.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.608           | 0.62          | 🔴0.98     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.602           | 0.583         | 🟢1.03     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 2.671           | 0.686         | 🟢**3.9**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 2.731           | 1.447         | 🟢1.89     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.892           | 0.808         | 🟢2.34     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.428           | 0.432         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 2.566           | 1.425         | 🟢1.8      |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.06            | 1.1           | 🔴0.96     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.992           | 1.178         | 🟢2.54     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 4.834           | 3.347         | 🟢1.44     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.369           | 1.404         | 🔴0.97     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 1.809           | 1.834         | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 2.016           | 0.947         | 🟢2.13     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 6.149           | 3.483         | 🟢1.77     |
| 12965000     | LONDON          | 259              | 30,025,257 | 12.941          | 4.022         | 🟢3.22     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.262           | 4.284         | 🟢1.23     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.799           | 2.911         | 🔴0.96     |
| 14029313     | LONDON          | 724              | 30,074,554 | 4.826           | 1.624         | 🟢2.97     |
| 14334629     | LONDON          | 819              | 30,135,754 | 6.374           | 3.392         | 🟢1.88     |
| 14383540     | LONDON          | 722              | 30,059,751 | 7.177           | 3.026         | 🟢2.37     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.169           | 3.271         | 🔴0.97     |
| 15199017     | LONDON          | 866              | 30,028,395 | 5.503           | 2.786         | 🟢1.98     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.055           | 1.049         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.782           | 1.174         | 🟢1.52     |
| 15538827     | MERGE           | 823              | 29,981,465 | 5.963           | 3.448         | 🟢1.73     |
| 16146267     | MERGE           | 473              | 19,204,593 | 4.896           | 2.058         | 🟢2.38     |
| 17034869     | MERGE           | 93               | 8,450,250  | 2.283           | 0.954         | 🟢2.39     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 6.689           | 3.136         | 🟢2.13     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 10.66           | 6.441         | 🟢1.65     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 5.041           | 2.512         | 🟢2.01     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 1.67            | 1.101         | 🟢1.52     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 4.268           | 2.249         | 🟢1.9      |
| 19807137     | CANCUN          | 712              | 29,981,386 | 12.34           | 7.09          | 🟢1.74     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 3.636           | 1.477         | 🟢2.46     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.482           | 0.408         | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 2.469           | 1.309         | 🟢1.89     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 4.502           | 2.311         | 🟢1.95     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 7.935           | 6.121         | 🟢1.3      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 5.204           | 2.634         | 🟢1.98     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.441           | 0.301         | 🟢1.47     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 2.724           | 1.676         | 🟢1.62     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 4.613           | 1.413         | 🟢3.27     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.096           | 0.639         | 🟢1.72     |

- We are currently **~1.77 times faster than sequential execution** on average.
- The **max speed up is x3.9** for a large block with few dependencies.
- The **max slow down is x0.78** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
