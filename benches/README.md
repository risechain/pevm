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
| Raw Transfers   | 47,620           | 1,000,020,000 | 126.17          | 49.290        | 🟢2.56     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 209.88          | 45.681        | 🟢4.59     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 577.37          | 56.614        | 🟢**10.2** |

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
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.073           | 0.073         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.414           | 0.427         | 🔴0.97     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.622           | 1.613         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.187           | 0.19          | 🔴0.99     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.107           | 0.104         | 🟢1.02     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.088           | 0.09          | 🔴0.98     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.819           | 0.407         | 🟢2.01     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.707           | 0.347         | 🟢2.03     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.394           | 2.31          | 🟢1.04     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.065           | 0.835         | 🟢2.47     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.631           | 0.65          | 🔴0.97     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.847           | 0.683         | 🟢1.24     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.871           | 1.061         | 🟢3.65     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.7             | 2.27          | 🟢2.07     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.848           | 0.921         | 🟢3.09     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.751           | 0.753         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.326           | 2.801         | 🟢1.54     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.109           | 1.159         | 🔴0.96     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.799           | 2.013         | 🟢2.88     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.319          | 7.52          | 🟢1.37     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.721           | 1.754         | 🔴0.98     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.932           | 2.953         | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.647           | 1.573         | 🟢2.32     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.231          | 8.041         | 🟢1.52     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.675          | 6.956         | 🟢3.26     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.078           | 4.438         | 🟢1.82     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.085           | 3.205         | 🔴0.96     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.743           | 2.009         | 🟢**4.35** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.301          | 4.53          | 🟢2.72     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.169          | 4.052         | 🟢3.25     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.204           | 4.291         | 🔴0.98     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.353           | 3.074         | 🟢3.04     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.059           | 1.06          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.624           | 1.583         | 🟢1.66     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.68           | 4.562         | 🟢2.56     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.221           | 2.452         | 🟢3.35     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.006           | 1.978         | 🟢2.53     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.384          | 6.338         | 🟢2.11     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.186          | 6.846         | 🟢2.22     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.402          | 5.715         | 🟢1.82     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.114           | 1.161         | 🟢1.82     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.318           | 5.055         | 🟢1.84     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.747          | 7.458         | 🟢2.65     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.291           | 3.569         | 🟢2.32     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.234           | 0.938         | 🟢1.32     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.761           | 2.332         | 🟢2.04     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.763           | 5.024         | 🟢1.94     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.556          | 6.595         | 🟢1.75     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.388          | 5.886         | 🟢2.1      |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.818           | 0.474         | 🟢1.73     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.106           | 3.358         | 🟢1.82     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.92            | 2.807         | 🟢3.53     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.241           | 1.304         | 🟢1.72     |

- We are currently **~2.08 times faster than sequential execution** on average.
- The **max speed up is x4.35** for a large block with few dependencies.
- The **max slow down is x0.96** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
