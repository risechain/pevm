# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large layer 2 blocks. All blocks are in the CANCUN spec with no dependencies, and we benchmark with `snmalloc` as the global memory allocator to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal layer 2. However, it may be representative of application-specific layer 2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S      |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 149.86 ms            | 89.596 ms          | 🟢1.67     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 227.75 ms            | 80.314 ms          | 🟢2.84     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 563.70 ms            | 64.427 ms          | 🟢**8.77** |

## Ethereum Mainnet Blocks

This benchmark includes several transactions for each Ethereum hardfork that alters the EVM spec. We include blocks with high parallelism, highly inter-dependent blocks, and some random blocks to ensure we benchmark against all scenarios. It is also a good testing platform for aggressively running blocks to find race conditions if there are any.

The current hardcoded concurrency level is 8, which has performed best for Ethereum blocks thus far. Increasing it will improve results for blocks with more parallelism but hurt small or highly interdependent blocks due to thread overheads. Ideally, our static analysis will be smart enough to auto-tune this better.

To run the benchmark:

```sh
$ cargo bench --bench mainnet
```

To benchmark with profiling for development (preferably after commenting out the sequential run):

```sh
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --bench mainnet -- --bench
```

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8248 µs            | 4.3367 µs          | 🔴0.88     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.567 µs            | 109.35 µs          | 🔴**0.58** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.964 µs            | 114.11 µs          | 🔴0.81     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 798.92 µs            | 1.2938 ms          | 🔴0.62     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6771 ms            | 1.8431 ms          | 🔴0.91     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 349.49 µs            | 556.61 µs          | 🔴0.63     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 134.71 µs            | 118.33 µs          | 🟢1.14     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 118.96 µs            | 118.80 µs          | 🟢1        |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3056 ms            | 540.29 µs          | 🟢2.41     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 792.17 µs            | 370.30 µs          | 🟢2.14     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7457 ms            | 2.4250 ms          | 🟢1.13     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3399 ms            | 2.0079 ms          | 🔴0.67     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2433 ms            | 1.3440 ms          | 🟢3.16     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9405 ms            | 2.4333 ms          | 🟢2.03     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5762 ms            | 1.3129 ms          | 🟢2.72     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 786.08 µs            | 945.10 µs          | 🔴0.83     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6424 ms            | 2.9577 ms          | 🟢1.57     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.3455 ms            | 3.1897 ms          | 🔴0.74     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3058 ms            | 3.1374 ms          | 🟢2.01     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.041 ms            | 8.4023 ms          | 🟢1.31     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0927 ms            | 3.8867 ms          | 🔴0.8      |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2891 ms            | 5.0598 ms          | 🔴0.85     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0932 ms            | 2.0177 ms          | 🟢2.03     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.487 ms            | 10.277 ms          | 🟢1.22     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.887 ms            | 8.1310 ms          | 🟢2.94     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8139 ms            | 6.9819 ms          | 🟢1.41     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5380 ms            | 8.7395 ms          | 🔴0.63     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.206 ms            | 2.7995 ms          | 🟢**3.65** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.866 ms            | 6.5065 ms          | 🟢2.13     |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.590 ms            | 5.8979 ms          | 🟢2.47     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.5939 ms            | 8.0884 ms          | 🔴0.82     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.956 ms            | 4.6443 ms          | 🟢2.36     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.282 µs            | 12.051 µs          | 🔴0.94     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1077 ms            | 1.9893 ms          | 🟢1.56     |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.517 ms            | 7.3227 ms          | 🟢1.85     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2659 ms            | 3.1063 ms          | 🟢2.98     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3119 ms            | 2.6058 ms          | 🟢2.04     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.099 ms            | 9.1730 ms          | 🟢1.54     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.622 ms            | 8.4551 ms          | 🟢1.97     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.969 ms            | 9.0630 ms          | 🟢1.21     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2209 ms            | 1.3641 ms          | 🟢1.63     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.174 ms            | 6.6647 ms          | 🟢1.53     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.758 ms            | 9.5825 ms          | 🟢2.27     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.7529 ms            | 4.4394 ms          | 🟢1.97     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3197 ms            | 1.1101 ms          | 🟢1.19     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1295 ms            | 2.9179 ms          | 🟢1.76     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.489 ms            | 6.2138 ms          | 🟢1.69     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.171 ms            | 6.9759 ms          | 🟢1.74     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.237 ms            | 7.8998 ms          | 🟢1.68     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 933.76 µs            | 618.70 µs          | 🟢1.51     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.3279 ms            | 3.9632 ms          | 🟢1.6      |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.417 ms            | 4.0660 ms          | 🟢2.56     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4146 ms            | 1.4922 ms          | 🟢1.62     |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.65** for a large block with few dependencies.
- The **max slow down is x0.58** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
