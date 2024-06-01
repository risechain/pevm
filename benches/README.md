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
| 46147        | FRONTIER        | 1                | 21,000     | 1.8640 µs            | 2.4376 µs          | 🔴0.76     |
| 930196       | FRONTIER        | 18               | 378,000    | 32.779 µs            | 93.287 µs          | 🔴**0.35** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 73.733 µs            | 108.32 µs          | 🔴0.68     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 426.04 µs            | 915.31 µs          | 🔴0.47     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6591 ms            | 1.8450 ms          | 🔴0.9      |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 199.93 µs            | 370.04 µs          | 🔴0.54     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 108.73 µs            | 113.78 µs          | 🔴0.96     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 91.569 µs            | 104.16 µs          | 🔴0.88     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 869.10 µs            | 480.39 µs          | 🟢1.81     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 738.62 µs            | 359.90 µs          | 🟢2.05     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.5444 ms            | 2.3599 ms          | 🟢1.08     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 685.15 µs            | 1.3305 ms          | 🔴0.51     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.9344 ms            | 1.3042 ms          | 🟢3.02     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.7059 ms            | 2.5106 ms          | 🟢1.87     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.9581 ms            | 1.1557 ms          | 🟢2.56     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 768.88 µs            | 941.37 µs          | 🔴0.82     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4222 ms            | 2.9142 ms          | 🟢1.52     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.2145 ms            | 2.1729 ms          | 🔴0.56     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.8912 ms            | 3.0482 ms          | 🟢1.93     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.636 ms            | 8.2981 ms          | 🟢1.28     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.8580 ms            | 2.6738 ms          | 🔴0.69     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.1123 ms            | 3.8794 ms          | 🔴0.8      |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.8036 ms            | 1.9403 ms          | 🟢1.96     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.305 ms            | 10.163 ms          | 🟢1.21     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.174 ms            | 8.1246 ms          | 🟢2.85     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.0338 ms            | 5.3862 ms          | 🟢1.49     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.1364 ms            | 6.1423 ms          | 🔴0.51     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.8572 ms            | 2.5856 ms          | 🟢**3.43** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.358 ms            | 6.5609 ms          | 🟢2.22     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.343 ms            | 5.6867 ms          | 🟢2.35     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.1521 ms            | 5.7683 ms          | 🔴0.72     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.3993 ms            | 3.6693 ms          | 🟢2.56     |
| 15537393     | LONDON          | 1                | 29,991,429 | 9.3825 µs            | 10.053 µs          | 🔴0.93     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9182 ms            | 1.9443 ms          | 🟢1.5      |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.976 ms            | 6.3510 ms          | 🟢1.89     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.4067 ms            | 2.9689 ms          | 🟢2.83     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1052 ms            | 2.5838 ms          | 🟢1.98     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.650 ms            | 9.0715 ms          | 🟢1.5      |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.194 ms            | 8.2998 ms          | 🟢1.83     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.684 ms            | 8.9016 ms          | 🟢1.2      |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1412 ms            | 1.3498 ms          | 🟢1.59     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.4354 ms            | 6.6381 ms          | 🟢1.42     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 20.223 ms            | 8.5927 ms          | 🟢2.35     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.4851 ms            | 4.4075 ms          | 🟢1.93     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2723 ms            | 1.1026 ms          | 🟢1.15     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.9008 ms            | 2.8725 ms          | 🟢1.71     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.034 ms            | 6.0928 ms          | 🟢1.65     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.827 ms            | 6.8582 ms          | 🟢1.72     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.659 ms            | 7.8420 ms          | 🟢1.61     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 848.47 µs            | 611.94 µs          | 🟢1.39     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.0401 ms            | 3.8873 ms          | 🟢1.55     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.126 ms            | 4.0239 ms          | 🟢2.52     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.2899 ms            | 1.4863 ms          | 🟢1.54     |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.43 (-6.27 ms)** for a large block with few dependencies.
- The **max slow down is x0.35 (+60.51 µs)** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.
