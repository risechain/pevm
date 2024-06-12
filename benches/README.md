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
| Raw Transfers   | 47,620           | 1,000,020,000 | 126.44 ms            | 51.736 ms          | 🟢2.44      |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 207.80 ms            | 62.535 ms          | 🟢3.32      |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 625.40 ms            | 60.435 ms          | 🟢**10.35** |

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
| 46147        | FRONTIER        | 1                | 21,000     | 2.2433 µs            | 2.2372 µs          | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 30.743 µs            | 30.672 µs          | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 73.264 µs            | 73.022 µs          | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 406.18 µs            | 433.44 µs          | 🔴**0.94** |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6404 ms            | 1.6559 ms          | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 181.70 µs            | 189.97 µs          | 🔴0.96     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 108.55 µs            | 103.44 µs          | 🟢1.05     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.628 µs            | 90.867 µs          | 🔴0.96     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 805.64 µs            | 413.07 µs          | 🟢1.95     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 713.19 µs            | 351.49 µs          | 🟢2.03     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.3222 ms            | 2.2261 ms          | 🟢1.04     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.0141 ms            | 837.09 µs          | 🟢2.41     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 605.69 µs            | 638.72 µs          | 🔴0.95     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 3.8082 ms            | 1.0601 ms          | 🟢3.59     |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.6853 ms            | 2.2463 ms          | 🟢2.09     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 2.7747 ms            | 930.87 µs          | 🟢2.98     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 751.08 µs            | 754.37 µs          | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.2131 ms            | 2.7091 ms          | 🟢1.56     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.0798 ms            | 1.1330 ms          | 🔴0.95     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 5.6601 ms            | 1.9896 ms          | 🟢2.84     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.137 ms            | 7.2262 ms          | 🟢1.4      |
| 12300570     | BERLIN          | 687              | 14,934,316 | 1.6921 ms            | 1.7446 ms          | 🔴0.97     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8332 ms            | 2.8679 ms          | 🔴0.99     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.5589 ms            | 1.5686 ms          | 🟢2.27     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 11.978 ms            | 7.6824 ms          | 🟢1.56     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.248 ms            | 6.8704 ms          | 🟢3.24     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 7.8150 ms            | 4.3102 ms          | 🟢1.81     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 3.0323 ms            | 3.1633 ms          | 🔴0.96     |
| 14029313     | LONDON          | 724              | 30,074,554 | 8.4606 ms            | 2.2192 ms          | 🟢**3.81** |
| 14334629     | LONDON          | 819              | 30,135,754 | 11.971 ms            | 4.5977 ms          | 🟢2.6      |
| 14383540     | LONDON          | 722              | 30,059,751 | 12.805 ms            | 4.0049 ms          | 🟢3.2      |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.0195 ms            | 4.1461 ms          | 🔴0.97     |
| 15199017     | LONDON          | 866              | 30,028,395 | 9.1372 ms            | 3.2395 ms          | 🟢2.82     |
| 15537393     | LONDON          | 1                | 29,991,429 | 1.0746 ms            | 1.0721 ms          | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.5436 ms            | 1.4998 ms          | 🟢1.7      |
| 15538827     | MERGE           | 823              | 29,981,465 | 11.449 ms            | 4.5259 ms          | 🟢2.53     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.0222 ms            | 2.5060 ms          | 🟢3.2      |
| 17034869     | MERGE           | 93               | 8,450,250  | 4.8682 ms            | 1.9033 ms          | 🟢2.56     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.075 ms            | 6.1074 ms          | 🟢2.14     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.769 ms            | 6.9143 ms          | 🟢2.14     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.132 ms            | 5.4809 ms          | 🟢1.85     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.1164 ms            | 1.1623 ms          | 🟢1.82     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 8.9554 ms            | 4.8785 ms          | 🟢1.84     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 19.438 ms            | 7.4685 ms          | 🟢2.6      |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.0468 ms            | 3.3993 ms          | 🟢2.37     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.1987 ms            | 899.84 µs          | 🟢1.33     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 4.6459 ms            | 2.2514 ms          | 🟢2.06     |
| 19932148     | CANCUN          | 227              | 14,378,808 | 9.4816 ms            | 4.8401 ms          | 🟢1.96     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.350 ms            | 6.5620 ms          | 🟢1.73     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.104 ms            | 5.6989 ms          | 🟢2.12     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 803.66 µs            | 489.53 µs          | 🟢1.64     |
| 19933597     | CANCUN          | 154              | 12,788,678 | 5.9631 ms            | 3.2357 ms          | 🟢1.84     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 9.6455 ms            | 2.7926 ms          | 🟢3.45     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.1707 ms            | 1.2397 ms          | 🟢1.75     |

- We are currently **~2.07 times faster than sequential execution** on average.
- The **max speed up is x3.81** for a large block with few dependencies.
- The **max slow down is x0.94** for a small block.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
