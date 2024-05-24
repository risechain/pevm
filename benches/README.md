# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7195 µs            | 5.6798 µs          | 1.53     |
| 930196       | FRONTIER        | 18               | 378,000    | 66.256 µs            | 135.11 µs          | 2.04     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 94.166 µs            | 150.94 µs          | 1.6      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 826.43 µs            | 1.6849 ms          | 2.04     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6355 ms            | 1.8810 ms          | 1.15     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 355.02 µs            | 736.05 µs          | 2.07     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 140.86 µs            | 156.92 µs          | 1.11     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.68 µs            | 155.43 µs          | 1.25     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3204 ms            | 780.51 ms          | **0.59** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 800.37 µs            | 418.12 µs          | **0.52** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7994 ms            | 4.2474 ms          | 1.52     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3237 ms            | 2.8064 ms          | 2.12     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2146 ms            | 1.7005 ms          | **0.4**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9444 ms            | 2.7751 ms          | **0.56** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6582 ms            | 2.0661 ms          | **0.56** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 798.28 µs            | 1.0040 ms          | 1.26     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6171 ms            | 4.2607 ms          | **0.92** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2594 ms            | 4.4540 ms          | 1.97     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.4059 ms            | 4.8857 ms          | **0.76** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.064 ms            | 11.049 ms          | **1**    |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2895 ms            | 6.5215 ms          | 1.52     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.621 ms            | 12.287 ms          | **0.97** |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.175 ms            | 11.248 ms          | **0.47** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8937 ms            | 8.9056 ms          | **0.9**  |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.752 µs            | 26.093 µs          | 2.22     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1672 ms            | 2.6285 ms          | **0.83** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.4205 ms            | 4.7082 ms          | **0.5**  |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3730 ms            | 3.6343 ms          | **0.68** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.266 ms            | 13.148 ms          | **0.92** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.919 ms            | 11.996 ms          | **0.71** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.156 ms            | 13.243 ms          | 1.19     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2896 ms            | 1.5022 ms          | **0.66** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.206 ms            | 9.9040 ms          | **0.97** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
