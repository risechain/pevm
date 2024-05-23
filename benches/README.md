# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8287 µs            | 5.5733 µs          | 1.46     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.940 µs            | 139.75 µs          | 2.15     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 93.614 µs            | 164.07 µs          | 1.75     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 838.93 µs            | 1.8230 ms          | 2.17     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6387 ms            | 2.0082 ms          | 1.23     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 359.42 µs            | 764.05 µs          | 2.13     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 140.88 µs            | 172.13 µs          | 1.22     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 121.18 µs            | 179.89 µs          | 1.48     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3181 ms            | 1.0411 ms          | **0.79** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 787.05 µs            | 521.20 µs          | **0.66** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6808 ms            | 5.5567 ms          | 2.07     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3043 ms            | 2.6933 ms          | 2.06     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1378 ms            | 1.8840 ms          | **0.46** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8241 ms            | 3.3959 ms          | **0.7**  |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5995 ms            | 2.8441 ms          | **0.79** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 798.62 µs            | 1.2262 ms          | 1.54     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4361 ms            | 5.0595 ms          | 1.14     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2561 ms            | 4.3905 ms          | 1.95     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2368 ms            | 5.9908 ms          | **0.96** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.626 ms            | 13.665 ms          | 1.29     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2176 ms            | 7.2764 ms          | 1.73     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.240 ms            | 15.705 ms          | 1.28     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.428 ms            | 14.426 ms          | **0.62** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7338 ms            | 9.3833 ms          | **0.96** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.503 µs            | 25.737 µs          | 2.24     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0390 ms            | 3.1048 ms          | 1.02     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.1856 ms            | 5.3110 ms          | **0.58** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1869 ms            | 4.6718 ms          | **0.9**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.708 ms            | 16.852 ms          | 1.23     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.557 ms            | 11.558 ms          | **0.7**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.823 ms            | 15.169 ms          | 1.4      |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2217 ms            | 2.3642 ms          | 1.06     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9094 ms            | 12.136 ms          | 1.22     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
