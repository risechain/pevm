# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6994 µs            | 5.5960 µs          | 1.48     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.198 µs            | 149.88 µs          | 2.37     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 42.667 µs            | 77.412 µs          | 1.81     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 834.81 µs            | 2.7004 ms          | 3.23     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6388 ms            | 2.0610 ms          | 1.26     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 347.13 µs            | 895.27 µs          | 2.58     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 105.03 µs            | 140.51 µs          | 1.34     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 86.626 µs            | 169.22 µs          | 1.95     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1485 ms            | 1.2116 ms          | 1.05     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 337.62 µs            | 289.26 µs          | **0.86** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 811.01 µs            | 1.5602 ms          | 1.92     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.2958 ms            | 4.0587 ms          | 3.13     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8779 ms            | 1.1205 ms          | **0.6**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3155 ms            | 663.50 µs          | **0.5**  |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8026 ms            | 923.44 µs          | **0.51** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.150 µs            | 185.85 µs          | 2.47     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 868.93 µs            | 680.65 µs          | **0.78** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.1860 ms            | 8.2054 ms          | 3.75     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3151 ms            | 1.7492 ms          | **0.76** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.0877 ms            | 1.7544 ms          | **0.84** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.9969 ms            | 11.210 ms          | 3.74     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.0606 ms            | 1.4059 ms          | **0.46** |
| 12965000     | LONDON          | 259              | 30,025,257 | 4.9740 ms            | 2.3279 ms          | **0.47** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.6037 ms            | 23.759 ms          | 4.24     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.314 µs            | 26.081 µs          | 2.31     |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.9685 ms            | 4.5806 ms          | 2.33     |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.6294 ms            | 6.0750 ms          | 1.67     |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.6086 ms            | 827.58 µs          | **0.51** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.3202 ms            | 1.4181 ms          | **0.43** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.3923 ms            | 11.162 ms          | 2.07     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2437 ms            | 1.2534 ms          | **0.56** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 418.27 µs            | 358.52 µs          | **0.86** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9120 ms            | 4.2834 ms          | 1.47     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
