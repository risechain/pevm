# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup     |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ----------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 125.49          | 49.563        | 🟢2.53      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 207.45          | 53.502        | 🟢3.88      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 579.13          | 57.840        | 🟢**10.01** |

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
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.416           | 0.423         | 🔴0.98     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.648           | 1.691         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.183           | 0.189         | 🔴0.97     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.107           | 0.104         | 🟢1.02     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.088           | 0.09          | 🔴0.97     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 0.826           | 0.416         | 🟢1.98     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.716           | 0.347         | 🟢2.06     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.414           | 2.327         | 🟢1.04     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.062           | 0.855         | 🟢2.41     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.621           | 0.635         | 🔴0.98     |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 0.841           | 0.686         | 🟢1.23     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.865           | 1.073         | 🟢3.6      |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.719           | 2.28          | 🟢2.07     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.829           | 0.933         | 🟢3.03     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.75            | 0.752         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.316           | 2.811         | 🟢1.54     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.094           | 1.134         | 🔴0.96     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.765           | 2.03          | 🟢2.84     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.407          | 7.558         | 🟢1.38     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.698           | 1.732         | 🔴0.98     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.908           | 2.96          | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.661           | 1.583         | 🟢2.31     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.285          | 7.916         | 🟢1.55     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.793          | 6.932         | 🟢3.29     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.968           | 4.284         | 🟢1.86     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.032           | 3.113         | 🔴0.97     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.68            | 2.167         | 🟢**4.01** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.348          | 4.646         | 🟢2.66     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.077          | 4.111         | 🟢3.18     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.116           | 4.186         | 🔴0.98     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.345           | 3.201         | 🟢2.92     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.062           | 1.064         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.759           | 1.581         | 🟢1.74     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.758          | 4.521         | 🟢2.6      |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.325           | 2.479         | 🟢3.36     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.046           | 2.025         | 🟢2.49     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.384          | 6.376         | 🟢2.1      |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.189          | 6.873         | 🟢2.21     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.598          | 5.662         | 🟢1.87     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.118           | 1.166         | 🟢1.82     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.223           | 5.092         | 🟢1.81     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.992          | 7.635         | 🟢2.62     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.312           | 3.52          | 🟢2.36     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.237           | 0.939         | 🟢1.32     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.745           | 2.341         | 🟢2.03     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.749           | 5.082         | 🟢1.92     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.627          | 6.55          | 🟢1.78     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.36           | 5.946         | 🟢2.08     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.819           | 0.475         | 🟢1.73     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.097           | 3.363         | 🟢1.81     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.013          | 2.887         | 🟢3.47     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.246           | 1.303         | 🟢1.72     |

- We are currently **~2.08 times faster than sequential execution** on average.
- The **max speed up is x4.01** for a large block with few dependencies.
- The **max slow down is x0.96** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
