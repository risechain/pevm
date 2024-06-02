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
| Raw Transfers   | 47,620           | 1,000,020,000 | 132.14 ms            | 84.002 ms          | 🟢1.57     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 223.58 ms            | 74.631 ms          | 🟢3        |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 585.76 ms            | 59.806 ms          | 🟢**9.79** |

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
| 46147        | FRONTIER        | 1                | 21,000     | --                   | --                 | --         |
| 930196       | FRONTIER        | 18               | 378,000    | --                   | --                 | --         |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 82.440 µs            | 106.08 µs          | 🔴0.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 503.17 µs            | 919.14 µs          | 🔴**0.55** |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.8091 ms            | 1.7680 ms          | 🟢1.02     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 227.98 µs            | 364.94 µs          | 🔴0.62     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 120.11 µs            | 110.45 µs          | 🟢1.09     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 98.577 µs            | 100.53 µs          | 🔴0.98     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 950.94 µs            | 487.07 µs          | 🟢1.95     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 774.11 µs            | 365.14 µs          | 🟢2.12     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6175 ms            | 2.3252 ms          | 🟢1.13     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 763.30 µs            | 1.3913 ms          | 🔴0.55     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.0209 ms            | 1.2946 ms          | 🟢3.11     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9032 ms            | 2.5108 ms          | 🟢1.95     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.1071 ms            | 1.1610 ms          | 🟢2.68     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 790.51 µs            | 938.52 µs          | 🔴0.84     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5722 ms            | 2.9080 ms          | 🟢1.57     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.3812 ms            | 2.2089 ms          | 🔴0.63     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.1667 ms            | 3.0321 ms          | 🟢2.03     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.870 ms            | 8.2761 ms          | 🟢1.31     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 2.0183 ms            | 2.6703 ms          | 🔴0.76     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.2803 ms            | 3.8962 ms          | 🔴0.84     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.8917 ms            | 1.9262 ms          | 🟢2.02     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.361 ms            | 10.131 ms          | 🟢1.22     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.636 ms            | 8.0108 ms          | 🟢2.95     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.3957 ms            | 5.5040 ms          | 🟢1.53     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.4195 ms            | 6.2311 ms          | 🔴0.55     |
| 14029313     | LONDON          | 724              | 30,074,554 | 9.2000 ms            | 2.6111 ms          | 🟢**3.52** |
| 14334629     | LONDON          | 819              | 30,135,754 | 12.774 ms            | 6.5636 ms          | 🟢1.95     |
| 14383540     | LONDON          | 722              | 30,059,751 | 13.751 ms            | 5.6883 ms          | 🟢2.42     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.4907 ms            | 5.8467 ms          | 🔴0.77     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.8056 ms            | 3.7087 ms          | 🟢2.64     |
| 15537393     | LONDON          | 1                | 29,991,429 | --                   | --                 | --         |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9652 ms            | 1.9256 ms          | 🟢1.54     |
| 15538827     | MERGE           | 823              | 29,981,465 | 12.356 ms            | 6.4055 ms          | 🟢1.93     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.6888 ms            | 3.0134 ms          | 🟢2.88     |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2219 ms            | 2.5590 ms          | 🟢2.04     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.967 ms            | 9.0103 ms          | 🟢1.55     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 15.539 ms            | 8.3322 ms          | 🟢1.86     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.906 ms            | 8.7861 ms          | 🟢1.24     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1747 ms            | 1.3486 ms          | 🟢1.61     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.6796 ms            | 6.6398 ms          | 🟢1.46     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 20.704 ms            | 8.6193 ms          | 🟢2.4      |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.6598 ms            | 4.3936 ms          | 🟢1.97     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2920 ms            | 1.0858 ms          | 🟢1.19     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0479 ms            | 2.8569 ms          | 🟢1.77     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.248 ms            | 6.0747 ms          | 🟢1.69     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.015 ms            | 6.8509 ms          | 🟢1.75     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.932 ms            | 7.8092 ms          | 🟢1.66     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 881.36 µs            | 611.92 µs          | 🟢1.44     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1728 ms            | 3.8731 ms          | 🟢1.59     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.441 ms            | 3.9860 ms          | 🟢2.62     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3653 ms            | 1.4760 ms          | 🟢1.6      |

- We are currently **~1.7 times faster than sequential execution** on average.
- The **max speed up is x3.52** for a large block with few dependencies.
- The **max slow down is x0.55** for a small block with many dependencies.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.

(\*) Blocks marked with `--` are small blocks that PEVM falls back to sequential execution. Spawning scoped threads and dropping the multi-version data structure alone may take longer than executing the whole block sequentially. Currently, these blocks either:

- Have fewer than two transactions.
- Use less than 378,000 gas.
