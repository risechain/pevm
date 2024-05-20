# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7552 µs            | 5.6156 µs          | 1.5      |
| 930196       | FRONTIER        | 18               | 378,000    | 63.269 µs            | 289.97 µs          | 4.58     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 42.093 µs            | 197.21 µs          | 4.69     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 808.22 µs            | 2.9118 ms          | 3.6      |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6318 ms            | 2.8028 ms          | 1.72     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 347.18 µs            | 957.57 µs          | 2.76     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 104.59 µs            | 239.22 µs          | 2.29     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 88.488 µs            | 277.52 µs          | 3.14     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1428 ms            | 1.2671 ms          | 1.11     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 337.21 µs            | 345.52 µs          | 1.02     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 819.07 µs            | 1.5366 ms          | 1.88     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3007 ms            | 5.0520 ms          | 3.88     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8652 ms            | 1.1009 ms          | **0.59** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3304 ms            | 678.11 µs          | **0.51** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8156 ms            | 943.71 µs          | **0.52** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.649 µs            | 207.35 µs          | 2.74     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 866.90 µs            | 701.36 µs          | **0.81** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.1473 ms            | 10.038 ms          | 4.67     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3129 ms            | 1.7966 ms          | **0.78** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.0910 ms            | 1.7041 ms          | **0.81** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.9694 ms            | 12.930 ms          | 4.35     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.0700 ms            | 1.4091 ms          | **0.46** |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.0290 ms            | 2.3194 ms          | **0.46** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.6042 ms            | 27.503 ms          | 4.91     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.445 µs            | 25.632 µs          | 2.24     |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.9843 ms            | 4.4324 ms          | 2.23     |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.6328 ms            | 6.6934 ms          | 1.84     |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.5994 ms            | 831.16 µs          | **0.52** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.3120 ms            | 1.4026 ms          | **0.42** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.3629 ms            | 12.639 ms          | 2.36     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2484 ms            | 1.2597 ms          | **0.56** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 419.47 µs            | 379.51 µs          | **0.9**  |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9190 ms            | 4.6534 ms          | 1.59     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
