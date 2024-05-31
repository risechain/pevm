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
| Raw Transfers   | 47,620           | 1,000,020,000 | 152.00 ms            | 110.91 ms          | 🟢1.37     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 226.13 ms            | 83.037 ms          | 🟢2.72     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 611.90 ms            | 65.575 ms          | 🟢**9.33** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 3.7264 µs            | 5.2812 µs          | 🔴0.71     |
| 930196       | FRONTIER        | 18               | 378,000    | 67.483 µs            | 121.71 µs          | 🔴**0.55** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.885 µs            | 117.47 µs          | 🔴0.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 831.17 µs            | 1.4406 ms          | 🔴0.58     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6581 ms            | 1.8242 ms          | 🔴0.91     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 352.97 µs            | 608.24 µs          | 🔴0.58     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 137.00 µs            | 119.45 µs          | 🟢1.15     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 119.93 µs            | 125.34 µs          | 🔴0.96     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3025 ms            | 629.48 µs          | 🟢2.07     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 778.10 µs            | 375.93 µs          | 🟢2.07     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6613 ms            | 2.3947 ms          | 🟢1.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3404 ms            | 2.2942 ms          | 🔴0.58     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1136 ms            | 1.3432 ms          | 🟢3.06     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.7986 ms            | 2.4169 ms          | 🟢1.99     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5162 ms            | 1.3529 ms          | 🟢2.6      |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 779.17 µs            | 947.10 µs          | 🔴0.82     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4057 ms            | 2.8470 ms          | 🟢1.55     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2161 ms            | 3.5504 ms          | 🔴0.62     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.1995 ms            | 3.1050 ms          | 🟢2        |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.582 ms            | 8.1436 ms          | 🟢1.3      |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0510 ms            | 4.1109 ms          | 🔴0.74     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2457 ms            | 5.3983 ms          | 🔴0.79     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0034 ms            | 2.0209 ms          | 🟢1.98     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.162 ms            | 10.035 ms          | 🟢1.21     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.380 ms            | 8.0404 ms          | 🟢2.91     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7555 ms            | 7.6137 ms          | 🟢1.28     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.6430 ms            | 9.8372 ms          | 🔴0.57     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.021 ms            | 2.9126 ms          | 🟢**3.44** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.426 ms            | 6.5178 ms          | 🟢2.06     |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.281 ms            | 6.0899 ms          | 🟢2.35     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4943 ms            | 8.9985 ms          | 🔴0.72     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.719 ms            | 4.9329 ms          | 🟢2.17     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.334 µs            | 13.009 µs          | 🔴0.87     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0390 ms            | 1.9190 ms          | 🟢1.58     |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.227 ms            | 7.6943 ms          | 🟢1.72     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.0859 ms            | 3.3753 ms          | 🟢2.69     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1494 ms            | 2.5570 ms          | 🟢2.01     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.664 ms            | 8.9414 ms          | 🟢1.53     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.400 ms            | 8.1751 ms          | 🟢2.01     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.625 ms            | 8.6241 ms          | 🟢1.23     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2231 ms            | 1.3673 ms          | 🟢1.63     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8923 ms            | 6.5905 ms          | 🟢1.5      |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.172 ms            | 9.9801 ms          | 🟢2.12     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.5034 ms            | 4.4003 ms          | 🟢1.93     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2803 ms            | 1.0698 ms          | 🟢1.2      |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.9881 ms            | 2.8808 ms          | 🟢1.73     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.126 ms            | 6.0786 ms          | 🟢1.67     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.875 ms            | 6.9549 ms          | 🟢1.71     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.803 ms            | 7.7118 ms          | 🟢1.66     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 920.43 µs            | 612.35 µs          | 🟢1.5      |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1262 ms            | 3.9311 ms          | 🟢1.56     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.119 ms            | 4.0133 ms          | 🟢2.52     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3122 ms            | 1.4134 ms          | 🟢1.64     |

- We are currently **~1.6 times faster than sequential execution** on average.
- The **max speed up is x3.44** for a large block with few dependencies.
- The **max slow down is x0.55** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
