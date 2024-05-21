# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8453 µs            | 5.6876 µs          | 1.48     |
| 930196       | FRONTIER        | 18               | 378,000    | 65.446 µs            | 149.87 µs          | 2.29     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 95.122 µs            | 168.35 µs          | 1.77     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 850.33 µs            | 2.0174 ms          | 2.37     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6625 ms            | 2.0523 ms          | 1.23     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 356.31 µs            | 820.41 µs          | 2.3      |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 141.35 µs            | 176.27 µs          | 1.25     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.70 µs            | 186.45 µs          | 1.5      |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3132 ms            | 1.2231 ms          | **0.93** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 799.85 µs            | 555.13 µs          | **0.69** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7093 ms            | 5.7316 ms          | 2.12     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3492 ms            | 2.9597 ms          | 2.19     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1934 ms            | 1.8965 ms          | **0.45** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9858 ms            | 3.5461 ms          | **0.71** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5976 ms            | 2.9710 ms          | **0.83** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 795.66 µs            | 1.2357 ms          | 1.55     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5283 ms            | 5.1635 ms          | 1.14     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2290 ms            | 4.6550 ms          | 2.09     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3133 ms            | 6.0567 ms          | **0.96** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.743 ms            | 13.637 ms          | 1.27     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2707 ms            | 7.7505 ms          | 1.81     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.369 ms            | 15.930 ms          | 1.29     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.920 ms            | 14.621 ms          | **0.61** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7851 ms            | 10.406 ms          | 1.06     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.823 µs            | 26.065 µs          | 2.2      |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1400 ms            | 4.6428 ms          | 1.48     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2518 ms            | 5.4956 ms          | **0.59** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2216 ms            | 4.7381 ms          | **0.91** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.994 ms            | 16.896 ms          | 1.21     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.784 ms            | 11.785 ms          | **0.7**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.842 ms            | 15.366 ms          | 1.42     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2512 ms            | 2.3684 ms          | 1.05     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9433 ms            | 12.313 ms          | 1.24     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
