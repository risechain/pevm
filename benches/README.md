# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard cloud services on which operators tend to run nodes.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block. All state needed is loaded into memory before execution and we pick `snmalloc` as the global memory allocator.

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how PEVM aids in building and syncing large blocks going forward. This explores performance for large L2 blocks. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

```sh
$ cargo bench --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | Speedup    |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 126.02 ms            | 86.880 ms          | 🟢1.45     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 220.59 ms            | 76.853 ms          | 🟢2.87     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 618.72 ms            | 61.903 ms          | 🟢**9.99** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.0208 µs            | 1.9863 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 25.424 µs            | 25.348 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 69.583 µs            | 69.767 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 355.81 µs            | 385.09 µs          | 🔴0.92     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.4326 ms            | 1.4475 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 160.89 µs            | 174.35 µs          | 🔴0.92     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 100.22 µs            | 104.57 µs          | 🔴0.96     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 82.385 µs            | 89.778 µs          | 🔴0.92     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 749.75 µs            | 422.46 µs          | 🟢1.77     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 690.97 µs            | 347.00 µs          | 🟢1.99     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.2764 ms            | 2.2102 ms          | 🟢1.03     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 1.9644 ms            | 849.65 µs          | 🟢2.31     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 575.68 µs            | 604.92 µs          | 🔴0.95     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.7527 ms            | 1.0692 ms          | 🟢3.51     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6242 ms            | 2.2535 ms          | 🟢2.05     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.6737 ms            | 936.44 µs          | 🟢2.86     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 736.94 µs            | 736.99 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.1826 ms            | 2.7160 ms          | 🟢1.54     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0106 ms            | 1.0821 ms          | 🔴0.93     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.5803 ms            | 1.9982 ms          | 🟢2.79     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 9.9300 ms            | 7.2656 ms          | 🟢1.37     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.5855 ms            | 1.6708 ms          | 🔴0.95     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.7480 ms            | 2.8248 ms          | 🔴0.97     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.4878 ms            | 1.5580 ms          | 🟢2.24     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.864 ms            | 7.6529 ms          | 🟢1.55     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.289 ms            | 6.7998 ms          | 🟢3.28     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.5147 ms            | 4.2493 ms          | 🟢1.77     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 2.6716 ms            | 2.8612 ms          | 🔴0.93     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.2296 ms            | 2.2740 ms          | 🟢**3.62** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.737 ms            | 4.5579 ms          | 🟢2.58     |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.700 ms            | 4.0688 ms          | 🟢3.12     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 3.7040 ms            | 3.9272 ms          | 🔴0.94     |
| 15199017     | LONDON          | 866              | 30,028,395 | 8.8736 ms            | 3.2543 ms          | 🟢2.73     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0537 ms            | 1.0538 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5742 ms            | 1.4968 ms          | 🟢1.72     |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.332 ms            | 4.4899 ms          | 🟢2.52     |
| 16146267     | MERGE           | 473              | 19,204,593 | 7.8997 ms            | 2.5665 ms          | 🟢3.08     |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.8249 ms            | 1.9070 ms          | 🟢2.53     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 12.898 ms            | 6.0774 ms          | 🟢2.12     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.473 ms            | 7.4034 ms          | 🟢1.95     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.026 ms            | 5.4496 ms          | 🟢1.84     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1101 ms            | 1.1722 ms          | 🟢1.8      |
| 19638737     | CANCUN          | 381              | 15,932,416 | 8.8133 ms            | 5.1243 ms          | 🟢1.72     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.116 ms            | 12.357 ms          | 🟢1.55     |
| 19917570     | CANCUN          | 116              | 12,889,065 | 7.9932 ms            | 3.4039 ms          | 🟢2.35     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.1828 ms            | 899.27 µs          | 🟢1.32     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.5879 ms            | 2.2563 ms          | 🟢2.03     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.3353 ms            | 4.9020 ms          | 🟢1.9      |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.356 ms            | 6.5359 ms          | 🟢1.74     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 11.897 ms            | 5.7609 ms          | 🟢2.07     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 784.05 µs            | 498.99 µs          | 🟢1.57     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.8378 ms            | 3.2878 ms          | 🟢1.78     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.5918 ms            | 2.8143 ms          | 🟢3.41     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1623 ms            | 1.2589 ms          | 🟢1.72     |

- We are currently **~2 times faster than sequential execution** on average.
- The **max speed up is x3.62** for a large block with few dependencies.
- The **max slow down is x0.92** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
