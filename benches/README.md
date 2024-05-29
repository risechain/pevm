# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes several transactions for each Ethereum hardfork that alters the EVM spec. We include blocks with high parallelism, highly inter-dependent blocks, and some random blocks to ensure we bench against all scenarios. It is also a good testing platform for aggressively running blocks to find race conditions if there are any.

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
| 46147        | FRONTIER        | 1                | 21,000     | 3.8133 µs            | 5.5464 µs          | 🔴0.69     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.198 µs            | 124.53 µs          | 🔴**0.52** |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 90.579 µs            | 117.67 µs          | 🔴0.77     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 824.90 µs            | 1.4999 ms          | 🔴0.55     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6504 ms            | 1.8854 ms          | 🔴0.88     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 353.64 µs            | 627.91 µs          | 🔴0.56     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 134.05 µs            | 119.58 µs          | 🟢1.12     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 120.42 µs            | 125.91 µs          | 🔴0.96     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3193 ms            | 644.19 µs          | 🟢2.05     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 785.45 µs            | 385.63 µs          | 🟢2.04     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6464 ms            | 2.4256 ms          | 🟢1.09     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3198 ms            | 2.4326 ms          | 🔴0.54     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1367 ms            | 1.3499 ms          | 🟢3.06     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8366 ms            | 2.4461 ms          | 🟢1.98     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5213 ms            | 1.4171 ms          | 🟢2.48     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 783.39 µs            | 961.55 µs          | 🔴0.81     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4349 ms            | 2.8628 ms          | 🟢1.55     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2744 ms            | 3.7299 ms          | 🔴0.61     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2085 ms            | 3.1359 ms          | 🟢1.98     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.676 ms            | 8.1790 ms          | 🟢1.31     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0600 ms            | 4.3783 ms          | 🔴0.7      |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2419 ms            | 5.6387 ms          | 🔴0.75     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0665 ms            | 2.0490 ms          | 🟢1.98     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.218 ms            | 10.069 ms          | 🟢1.21     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.524 ms            | 8.1404 ms          | 🟢2.89     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.6725 ms            | 7.8234 ms          | 🟢1.24     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5385 ms            | 10.135 ms          | 🔴0.55     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.050 ms            | 2.9390 ms          | 🟢**3.42** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.423 ms            | 6.6662 ms          | 🟢2.01     |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.340 ms            | 6.1922 ms          | 🟢2.32     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.5816 ms            | 9.3410 ms          | 🔴0.7      |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.753 ms            | 5.0941 ms          | 🟢2.11     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.539 µs            | 13.292 µs          | 🔴0.87     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9696 ms            | 1.9209 ms          | 🟢1.55     |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.216 ms            | 7.6968 ms          | 🟢1.72     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.1232 ms            | 3.3758 ms          | 🟢2.7      |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1528 ms            | 2.5558 ms          | 🟢2.02     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.692 ms            | 9.0195 ms          | 🟢1.52     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.447 ms            | 8.2956 ms          | 🟢1.98     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.662 ms            | 8.8176 ms          | 🟢1.21     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2226 ms            | 1.3733 ms          | 🟢1.62     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8232 ms            | 6.6009 ms          | 🟢1.49     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.256 ms            | 10.197 ms          | 🟢2.08     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.5545 ms            | 4.3840 ms          | 🟢1.95     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2734 ms            | 1.0750 ms          | 🟢1.18     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0080 ms            | 2.9115 ms          | 🟢1.72     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.159 ms            | 6.0995 ms          | 🟢1.67     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.891 ms            | 6.9915 ms          | 🟢1.7      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.847 ms            | 7.7446 ms          | 🟢1.66     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 913.59 µs            | 621.96 µs          | 🟢1.47     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1444 ms            | 3.9447 ms          | 🟢1.56     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.222 ms            | 4.0232 ms          | 🟢2.54     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3129 ms            | 1.4174 ms          | 🟢1.63     |

- We are currently **~1.6 times faster than sequential execution** on average.
- The **max speed up is x3.42** for a large block with few dependencies.
- The **max slow down is x1.94** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

Intuitively, we have consistently been faster in recent eras and slower in early eras when most transactions were simple transfers that don't justify the parallel overheads. As it stands, syncing nodes can execute sequentially until Spurious Dragon before switching on PEVM. Ideally, PEVM would minimize the worst-case to under 25% overhead.

## Gigagas

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies, and we bench with `snmalloc` as the global memory allocator to measure the maximum speedup.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S      |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 149.74 ms            | 111.98 ms          | 🟢1.34     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 225.16 ms            | 85.193 ms          | 🟢2.64     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 563.92 ms            | 65.363 ms          | 🟢**8.63** |
