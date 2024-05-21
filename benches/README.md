# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7598 µs            | 5.5737 µs          | 1.48     |
| 930196       | FRONTIER        | 18               | 378,000    | 67.022 µs            | 150.29 µs          | 2.24     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 93.214 µs            | 169.09 µs          | 1.81     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 863.78 µs            | 2.0324 ms          | 2.35     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6427 ms            | 2.0442 ms          | 1.24     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 358.19 µs            | 816.29 µs          | 2.28     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 144.99 µs            | 176.30 µs          | 1.22     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.03 µs            | 186.98 µs          | 1.51     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3218 ms            | 1.1835 ms          | **0.9**  |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 800.20 µs            | 557.36 µs          | **0.7**  |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7253 ms            | 5.7607 ms          | 2.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3823 ms            | 2.9788 ms          | 2.15     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1923 ms            | 1.8938 ms          | **0.45** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 5.0092 ms            | 3.5871 ms          | **0.72** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6206 ms            | 2.8940 ms          | **0.8**  |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 796.25 µs            | 1.2319 ms          | 1.55     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5251 ms            | 5.1884 ms          | 1.15     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2830 ms            | 4.6345 ms          | 2.03     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3306 ms            | 6.0656 ms          | **0.96** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.875 ms            | 13.751 ms          | 1.26     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3463 ms            | 7.6926 ms          | 1.77     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.381 ms            | 15.906 ms          | 1.28     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.767 ms            | 14.455 ms          | **0.61** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8542 ms            | 10.588 ms          | 1.07     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.619 µs            | 25.857 µs          | 2.23     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0849 ms            | 4.5416 ms          | 1.47     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2895 ms            | 5.5947 ms          | **0.6**  |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2592 ms            | 4.7373 ms          | **0.9**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.939 ms            | 16.881 ms          | 1.21     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.715 ms            | 11.840 ms          | **0.71** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.834 ms            | 15.542 ms          | 1.43     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2480 ms            | 2.3544 ms          | 1.05     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9492 ms            | 12.324 ms          | 1.24     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
