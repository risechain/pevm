# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7750 µs            | 5.8496 µs          | 1.55     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.774 µs            | 155.56 µs          | 2.4      |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 94.905 µs            | 168.55 µs          | 1.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 852.62 µs            | 2.7520 ms          | 3.23     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6679 ms            | 2.0613 ms          | 1.24     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 354.56 µs            | 914.31 µs          | 2.58     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 138.00 µs            | 174.73 µs          | 1.27     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 123.62 µs            | 190.14 µs          | 1.54     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3246 ms            | 1.2279 ms          | **0.93** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 799.06 µs            | 554.84 µs          | **0.69** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6992 ms            | 5.8054 ms          | 2.15     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3278 ms            | 4.0763 ms          | 3.07     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2092 ms            | 1.8872 ms          | **0.45** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9707 ms            | 3.6866 ms          | **0.74** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5811 ms            | 3.0441 ms          | **0.85** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 797.10 µs            | 1.2395 ms          | 1.56     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4932 ms            | 5.1768 ms          | 1.15     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2655 ms            | 8.3221 ms          | 3.67     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2818 ms            | 6.1168 ms          | **0.97** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.690 ms            | 13.612 ms          | 1.27     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2344 ms            | 13.221 ms          | 3.12     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.303 ms            | 15.796 ms          | 1.28     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.621 ms            | 14.458 ms          | **0.61** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7756 ms            | 24.388 ms          | 2.49     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.788 µs            | 26.484 µs          | 2.25     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0668 ms            | 4.4905 ms          | 1.46     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2298 ms            | 6.8696 ms          | **0.74** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2229 ms            | 4.7299 ms          | **0.91** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.879 ms            | 16.833 ms          | 1.21     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.790 ms            | 16.173 ms          | **0.96** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.770 ms            | 15.389 ms          | 1.43     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2453 ms            | 2.3764 ms          | 1.06     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9263 ms            | 12.656 ms          | 1.27     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
