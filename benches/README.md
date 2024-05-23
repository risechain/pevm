# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7860 µs            | 5.4572 µs          | 1.44     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.195 µs            | 137.44 µs          | 2.17     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 95.554 µs            | 152.94 µs          | 1.6      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 816.00 µs            | 1.7053 ms          | 2.09     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6598 ms            | 1.8705 ms          | 1.13     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 354.58 µs            | 739.32 µs          | 2.09     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 138.25 µs            | 158.23 µs          | 1.14     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 123.24 µs            | 158.40 µs          | 1.29     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3254 ms            | 805.55 ms          | **0.61** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 801.17 µs            | 450.43 µs          | **0.56** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7463 ms            | 4.2092 ms          | 1.53     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3058 ms            | 2.8766 ms          | 2.2      |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1662 ms            | 1.6880 ms          | **0.41** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8606 ms            | 2.7813 ms          | **0.57** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6357 ms            | 1.9836 ms          | **0.55** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 793.72 µs            | 1.0105 ms          | 1.27     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4535 ms            | 4.1535 ms          | **0.93** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2361 ms            | 4.5756 ms          | 2.05     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2981 ms            | 4.9131 ms          | **0.78** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.665 ms            | 10.798 ms          | 1.01     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2192 ms            | 6.5084 ms          | 1.54     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.410 ms            | 12.147 ms          | **0.98** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.581 ms            | 11.228 ms          | **0.48** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8401 ms            | 9.1252 ms          | **0.93** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.721 µs            | 25.738 µs          | 2.2      |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0428 ms            | 2.5649 ms          | **0.84** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2526 ms            | 4.4598 ms          | **0.48** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2147 ms            | 3.6021 ms          | **0.69** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.806 ms            | 13.038 ms          | **0.94** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.577 ms            | 11.878 ms          | **0.72** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.777 ms            | 13.129 ms          | 1.22     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2536 ms            | 1.5001 ms          | **0.67** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8616 ms            | 9.7002 ms          | **0.98** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
