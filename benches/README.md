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
| Raw Transfers   | 47,620           | 1,000,020,000 | 152.26 ms            | 108.48 ms          | 🟢1.4      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 229.24 ms            | 84.142 ms          | 🟢2.72     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 580.26 ms            | 64.427 ms          | 🟢**9.01** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 3.7900 µs            | 4.3562 µs          | 🔴0.87     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.128 µs            | 112.36 µs          | 🔴**0.57** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.468 µs            | 115.94 µs          | 🔴0.79     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 806.13 µs            | 1.2977 ms          | 🔴0.62     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6635 ms            | 1.8322 ms          | 🔴0.91     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 346.92 µs            | 561.62 µs          | 🔴0.62     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 136.15 µs            | 116.89 µs          | 🟢1.16     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.10 µs            | 120.36 µs          | 🟢1.03     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3089 ms            | 607.79 µs          | 🟢2.15     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 779.83 µs            | 370.51 µs          | 🟢2.1      |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7189 ms            | 2.4568 ms          | 🟢1.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3051 ms            | 2.0052 ms          | 🔴0.65     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2064 ms            | 1.3624 ms          | 🟢3.09     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9520 ms            | 2.4270 ms          | 🟢2.04     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6400 ms            | 1.3142 ms          | 🟢2.77     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 766.89 µs            | 932.77 µs          | 🔴0.82     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5855 ms            | 2.9763 ms          | 🟢1.54     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2231 ms            | 3.2112 ms          | 🔴0.69     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3923 ms            | 3.1649 ms          | 🟢2.02     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.181 ms            | 8.4810 ms          | 🟢1.32     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0249 ms            | 3.9226 ms          | 🔴0.77     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3129 ms            | 5.0831 ms          | 🔴0.85     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.1176 ms            | 2.0500 ms          | 🟢2.01     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.610 ms            | 10.452 ms          | 🟢1.21     |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.042 ms            | 8.1918 ms          | 🟢2.93     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.9009 ms            | 7.1234 ms          | 🟢1.39     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5354 ms            | 9.0204 ms          | 🔴0.61     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.279 ms            | 2.8607 ms          | 🟢**3.59** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.878 ms            | 6.6221 ms          | 🟢2.1      |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.713 ms            | 5.9387 ms          | 🟢2.48     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4551 ms            | 8.2426 ms          | 🔴0.78     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.977 ms            | 4.7118 ms          | 🟢2.33     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.319 µs            | 11.930 µs          | 🔴0.95     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0745 ms            | 2.0090 ms          | 🟢1.53     |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.624 ms            | 7.5370 ms          | 🟢1.81     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.3223 ms            | 3.1876 ms          | 🟢2.92     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3967 ms            | 2.6650 ms          | 🟢2.03     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.284 ms            | 9.2860 ms          | 🟢1.54     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.589 ms            | 8.6186 ms          | 🟢1.92     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.092 ms            | 9.1445 ms          | 🟢1.21     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2230 ms            | 1.3648 ms          | 🟢1.63     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.244 ms            | 6.7937 ms          | 🟢1.51     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.769 ms            | 9.7227 ms          | 🟢2.24     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.9614 ms            | 4.5385 ms          | 🟢1.97     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3372 ms            | 1.1400 ms          | 🟢1.17     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1479 ms            | 2.9570 ms          | 🟢1.74     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.648 ms            | 6.2757 ms          | 🟢1.7      |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.206 ms            | 6.9867 ms          | 🟢1.75     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.350 ms            | 8.0318 ms          | 🟢1.66     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 940.56 µs            | 628.69 µs          | 🟢1.5      |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.4245 ms            | 4.0103 ms          | 🟢1.6      |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.563 ms            | 4.1226 ms          | 🟢2.56     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4557 ms            | 1.5453 ms          | 🟢1.59     |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.59** for a large block with few dependencies.
- The **max slow down is x0.57** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
