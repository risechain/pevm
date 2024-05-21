# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7735 µs            | 5.5761 µs          | 1.48     |
| 930196       | FRONTIER        | 18               | 378,000    | 65.304 µs            | 143.17 µs          | 2.19     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 93.731 µs            | 167.12 µs          | 1.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 839.41 µs            | 1.9379 ms          | 2.31     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6652 ms            | 2.0024 ms          | 1.2      |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 359.17 µs            | 797.55 µs          | 2.22     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 145.12 µs            | 178.53 µs          | 1.23     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 131.89 µs            | 183.32 µs          | 1.39     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3518 ms            | 1.1653 ms          | **0.86** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 805.31 µs            | 539.95 µs          | **0.67** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.8182 ms            | 5.6790 ms          | 2.02     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3379 ms            | 2.8175 ms          | 2.11     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2943 ms            | 1.8750 ms          | **0.44** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 5.0033 ms            | 3.4559 ms          | **0.69** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6742 ms            | 2.8915 ms          | **0.79** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 792.65 µs            | 1.2147 ms          | 1.53     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6938 ms            | 5.2187 ms          | 1.11     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2779 ms            | 4.5469 ms          | 2        |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.4957 ms            | 6.0502 ms          | **0.93** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.330 ms            | 13.718 ms          | 1.21     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.4157 ms            | 7.5328 ms          | 1.71     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.959 ms            | 16.173 ms          | 1.25     |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.580 ms            | 14.590 ms          | **0.59** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 10.053 ms            | 10.284 ms          | 1.02     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.583 µs            | 25.809 µs          | 2.23     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1785 ms            | 3.2075 ms          | 1.01     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.5522 ms            | 5.4430 ms          | **0.57** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.4960 ms            | 4.8504 ms          | **0.88** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.499 ms            | 17.309 ms          | 1.19     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 17.147 ms            | 12.125 ms          | **0.71** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.311 ms            | 15.657 ms          | 1.38     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2640 ms            | 2.3813 ms          | 1.05     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.419 ms            | 12.481 ms          | 1.2      |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
