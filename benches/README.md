# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | Speedup     |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ----------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 125.98 ms            | 85.587 ms          | 🟢1.47      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 218.52 ms            | 76.000 ms          | 🟢2.88      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 641.60 ms            | 62.659 ms          | 🟢**10.24** |

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

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | 1.9899 µs            | 2.0241 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 25.767 µs            | 25.788 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 69.253 µs            | 69.372 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 362.81 µs            | 379.57 µs          | 🔴0.96     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.4270 ms            | 1.4317 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 159.42 µs            | 168.72 µs          | 🔴0.94     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 99.086 µs            | 103.28 µs          | 🔴0.96     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 80.306 µs            | 88.929 µs          | 🔴**0.9**  |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 743.84 µs            | 414.58 µs          | 🟢1.79     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 699.66 µs            | 352.84 µs          | 🟢1.98     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.2650 ms            | 2.2078 ms          | 🟢1.03     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.9591 ms            | 847.13 µs          | 🟢2.31     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 553.58 µs            | 584.92 µs          | 🔴0.95     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.7527 ms            | 1.0577 ms          | 🟢3.55     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6728 ms            | 2.2397 ms          | 🟢2.09     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.6840 ms            | 935.99 µs          | 🟢2.87     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 748.80 µs            | 752.74 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.1517 ms            | 2.6818 ms          | 🟢1.55     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0024 ms            | 1.0488 ms          | 🔴0.96     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.5364 ms            | 1.9684 ms          | 🟢2.81     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 9.8660 ms            | 7.2200 ms          | 🟢1.37     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.6051 ms            | 1.6530 ms          | 🔴0.97     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.7179 ms            | 2.7634 ms          | 🔴0.98     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.4932 ms            | 1.5536 ms          | 🟢2.25     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.834 ms            | 7.6134 ms          | 🟢1.55     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.327 ms            | 6.7879 ms          | 🟢3.29     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.4934 ms            | 4.1985 ms          | 🟢1.78     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.6757 ms            | 2.7660 ms          | 🔴0.97     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.2611 ms            | 2.2744 ms          | 🟢**3.63** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.752 ms            | 4.5769 ms          | 🟢2.57     |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.547 ms            | 4.0043 ms          | 🟢3.13     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.6994 ms            | 3.8074 ms          | 🔴0.97     |
| 15199017     | LONDON          | 866              | 30,028,395 | 8.8885 ms            | 3.2154 ms          | 🟢2.76     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0676 ms            | 1.0721 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5276 ms            | 1.4977 ms          | 🟢1.69     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.127 ms            | 4.4701 ms          | 🟢2.49     |
| 16146267     | MERGE           | 473              | 19,204,593 | 7.8709 ms            | 2.5462 ms          | 🟢3.09     |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.7839 ms            | 1.8929 ms          | 🟢2.53     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 12.873 ms            | 6.0528 ms          | 🟢2.13     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.470 ms            | 7.3023 ms          | 🟢1.98     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.018 ms            | 5.4275 ms          | 🟢1.85     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.0907 ms            | 1.1688 ms          | 🟢1.79     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 8.7831 ms            | 4.9573 ms          | 🟢1.77     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.096 ms            | 7.7267 ms          | 🟢2.47     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.0229 ms            | 3.3865 ms          | 🟢2.37     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.1863 ms            | 894.90 µs          | 🟢1.33     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.6310 ms            | 2.2496 ms          | 🟢2.06     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.3628 ms            | 4.8749 ms          | 🟢1.92     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.254 ms            | 6.6025 ms          | 🟢1.7      |
| 19932810     | CANCUN          | 270              | 18,643,597 | 11.899 ms            | 5.6960 ms          | 🟢2.09     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 792.96 µs            | 492.37 µs          | 🟢1.61     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.8751 ms            | 3.2494 ms          | 🟢1.81     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.5495 ms            | 2.8106 ms          | 🟢3.4      |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1548 ms            | 1.2394 ms          | 🟢1.74     |

- We are currently **~2.05 times faster than sequential execution** on average.
- The **max speed up is x3.63** for a large block with few dependencies.
- The **max slow down is x0.9** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
