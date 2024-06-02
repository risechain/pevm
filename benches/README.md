# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large layer 2 blocks. All blocks are in the CANCUN spec with no dependencies, and we benchmark with `snmalloc` as the global memory allocator to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal layer 2. However, it may be representative of application-specific layer 2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S     |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | --------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 121.67 ms            | 88.781 ms          | 🟢1.37    |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 212.59 ms            | 78.252 ms          | 🟢2.72    |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 607.78 ms            | 66.073 ms          | 🟢**9.2** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 1.8647 µs            | 2.1573 µs          | 🔴0.86     |
| 930196       | FRONTIER        | 18               | 378,000    | 32.158 µs            | 91.521 µs          | 🔴**0.35** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 74.303 µs            | 105.61 µs          | 🔴0.7      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 442.00 µs            | 892.94 µs          | 🔴0.49     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6542 ms            | 1.7841 ms          | 🔴0.93     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 207.16 µs            | 355.25 µs          | 🔴0.58     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 110.10 µs            | 108.65 µs          | 🟢1.01     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 94.822 µs            | 98.143 µs          | 🔴0.97     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 880.51 µs            | 470.76 µs          | 🟢1.87     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 749.62 µs            | 356.34 µs          | 🟢2.1      |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.5849 ms            | 2.3718 ms          | 🟢1.09     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 690.69 µs            | 1.3272 ms          | 🔴0.52     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.0125 ms            | 1.3141 ms          | 🟢3.05     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8530 ms            | 2.5246 ms          | 🟢1.92     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.0166 ms            | 1.1289 ms          | 🟢2.67     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 763.69 µs            | 925.68 µs          | 🔴0.83     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5043 ms            | 2.9600 ms          | 🟢1.52     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.2350 ms            | 2.1025 ms          | 🔴0.59     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.0095 ms            | 3.0682 ms          | 🟢1.96     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.919 ms            | 8.5064 ms          | 🟢1.28     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.8716 ms            | 2.6026 ms          | 🔴0.72     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.1922 ms            | 3.8355 ms          | 🔴0.83     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.8420 ms            | 1.9452 ms          | 🟢1.98     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.387 ms            | 10.324 ms          | 🟢1.2      |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.695 ms            | 8.2036 ms          | 🟢2.89     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.1640 ms            | 5.4284 ms          | 🟢1.5      |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.1571 ms            | 6.0739 ms          | 🔴0.52     |
| 14029313     | LONDON          | 724              | 30,074,554 | 9.0601 ms            | 2.5649 ms          | 🟢**3.53** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.690 ms            | 6.6762 ms          | 🟢1.9      |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.779 ms            | 5.7110 ms          | 🟢2.41     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.2409 ms            | 5.6702 ms          | 🔴0.75     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.5984 ms            | 3.6427 ms          | 🟢2.63     |
| 15537393     | LONDON          | 1                | 29,991,429 | 9.3880 µs            | 9.7334 µs          | 🔴0.96     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0136 ms            | 1.9786 ms          | 🟢1.52     |
| 15538827     | MERGE           | 823              | 29,981,465 | 12.395 ms            | 6.4037 ms          | 🟢1.94     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.6773 ms            | 2.9954 ms          | 🟢2.9      |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2488 ms            | 2.6114 ms          | 🟢2.01     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.970 ms            | 9.2324 ms          | 🟢1.51     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.370 ms            | 8.4742 ms          | 🟢1.81     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.923 ms            | 9.0946 ms          | 🟢1.2      |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1660 ms            | 1.3479 ms          | 🟢1.61     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.7631 ms            | 6.7134 ms          | 🟢1.45     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 20.544 ms            | 8.7496 ms          | 🟢2.35     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.7190 ms            | 4.5236 ms          | 🟢1.93     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2927 ms            | 1.1181 ms          | 🟢1.16     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0005 ms            | 2.9025 ms          | 🟢1.72     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.320 ms            | 6.1795 ms          | 🟢1.67     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.094 ms            | 6.8512 ms          | 🟢1.77     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.985 ms            | 7.9690 ms          | 🟢1.63     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 857.86 µs            | 618.27 µs          | 🟢1.39     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1787 ms            | 3.9911 ms          | 🟢1.55     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.469 ms            | 4.0903 ms          | 🟢2.56     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3752 ms            | 1.5490 ms          | 🟢1.53     |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.53** for a large block with few dependencies.
- The **max slow down is x0.35** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
