# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S   |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6496 µs            | 5.4436 µs          | 149%    |
| 930196       | FRONTIER        | 18               | 378,000    | 62.674 µs            | 286.14 µs          | 457%    |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 41.649 µs            | 194.42 µs          | 467%    |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 784.99 µs            | 4.2547 ms          | 542%    |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6506 ms            | 2.8406 ms          | 172%    |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 347.01 µs            | 1.2492 ms          | 360%    |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 100.89 µs            | 235.72 µs          | 234%    |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 84.904 µs            | 276.92 µs          | 326%    |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1751 ms            | 1.4830 ms          | 126%    |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 341.37 µs            | 346.67 µs          | 102%    |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 811.59 µs            | 1.5576 ms          | 192%    |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.2566 ms            | 9.5493 ms          | 760%    |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8795 ms            | 1.0689 ms          | **57%** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3374 ms            | 665.23 µs          | **50%** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8338 ms            | 1.0370 ms          | **57%** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 74.179 µs            | 206.60 µs          | 279%    |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 881.15 µs            | 695.83 µs          | **79%** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.1098 ms            | 15.724 ms          | 745%    |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3608 ms            | 1.7288 ms          | **73%** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.1331 ms            | 1.7109 ms          | **80%** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8594 ms            | 19.317 ms          | 676%    |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.1168 ms            | 1.3981 ms          | **45%** |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.1462 ms            | 2.3374 ms          | **45%** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.6289 ms            | 43.856 ms          | 779%    |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.265 µs            | 25.582 µs          | 227%    |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.0362 ms            | 4.5019 ms          | 221%    |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.6401 ms            | 6.7815 ms          | 186%    |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.6501 ms            | 837.86 µs          | **51%** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.4239 ms            | 1.4138 ms          | **41%** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.4748 ms            | 16.632 ms          | 304%    |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.3336 ms            | 1.3040 ms          | **56%** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 407.13 µs            | 374.82 µs          | **92%** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9770 ms            | 4.6610 ms          | 157%    |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
