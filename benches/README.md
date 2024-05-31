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
| Raw Transfers   | 47,620           | 1,000,020,000 | 151.19 ms            | 108.32 ms          | 🟢1.4      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 228.82 ms            | 82.407 ms          | 🟢2.78     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 610.67 ms            | 65.746 ms          | 🟢**9.29** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 3.7797 µs            | 5.4457 µs          | 🔴0.69     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.907 µs            | 122.86 µs          | 🔴**0.53** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 92.528 µs            | 116.61 µs          | 🔴0.79     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 822.04 µs            | 1.4508 ms          | 🔴0.57     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6580 ms            | 1.8709 ms          | 🔴0.89     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 356.65 µs            | 610.05 µs          | 🔴0.58     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 135.56 µs            | 122.13 µs          | 🟢1.11     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 118.18 µs            | 125.33 µs          | 🔴0.94     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3043 ms            | 632.36 µs          | 🟢2.06     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 779.64 µs            | 373.73 µs          | 🟢2.09     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6434 ms            | 2.3990 ms          | 🟢1.1      |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3524 ms            | 2.3064 ms          | 🔴0.59     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1331 ms            | 1.3422 ms          | 🟢3.08     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8631 ms            | 2.4073 ms          | 🟢2.02     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5557 ms            | 1.3597 ms          | 🟢2.62     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 775.09 µs            | 950.07 µs          | 🔴0.82     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4154 ms            | 2.8339 ms          | 🟢1.56     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2274 ms            | 3.5746 ms          | 🔴0.62     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2018 ms            | 3.1707 ms          | 🟢1.96     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.723 ms            | 8.1108 ms          | 🟢1.32     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0539 ms            | 4.2237 ms          | 🔴0.72     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2524 ms            | 5.3863 ms          | 🔴0.79     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0173 ms            | 2.0181 ms          | 🟢1.99     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.197 ms            | 9.9974 ms          | 🟢1.22     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.348 ms            | 8.0176 ms          | 🟢2.91     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8159 ms            | 7.6552 ms          | 🟢1.28     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5943 ms            | 9.8964 ms          | 🔴0.63     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.087 ms            | 2.9252 ms          | 🟢**3.45** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.417 ms            | 6.5359 ms          | 🟢2.05     |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.271 ms            | 6.1156 ms          | 🟢2.33     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4418 ms            | 9.0465 ms          | 🔴0.71     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.757 ms            | 4.9873 ms          | 🟢2.16     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.493 µs            | 12.968 µs          | 🔴0.89     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9699 ms            | 1.9069 ms          | 🟢1.56     |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.219 ms            | 7.6873 ms          | 🟢1.72     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.0894 ms            | 3.2204 ms          | 🟢2.82     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1555 ms            | 2.5132 ms          | 🟢2.05     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.672 ms            | 8.9260 ms          | 🟢1.53     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.296 ms            | 8.1969 ms          | 🟢1.99     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.618 ms            | 8.6075 ms          | 🟢1.23     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2227 ms            | 1.3711 ms          | 🟢1.62     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.7861 ms            | 6.4509 ms          | 🟢1.52     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.128 ms            | 9.7196 ms          | 🟢2.17     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.4757 ms            | 4.3217 ms          | 🟢1.96     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2682 ms            | 1.0642 ms          | 🟢1.19     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0127 ms            | 2.8567 ms          | 🟢1.75     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.152 ms            | 6.0603 ms          | 🟢1.68     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.873 ms            | 6.9568 ms          | 🟢1.71     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.868 ms            | 7.6772 ms          | 🟢1.68     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 913.91 µs            | 608.82 µs          | 🟢1.5      |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1184 ms            | 3.8749 ms          | 🟢1.58     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.140 ms            | 3.9600 ms          | 🟢2.56     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3148 ms            | 1.4024 ms          | 🟢1.65     |

- We are currently **~1.6 times faster than sequential execution** on average.
- The **max speed up is x3.45** for a large block with few dependencies.
- The **max slow down is x0.53** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
