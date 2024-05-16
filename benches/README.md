# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S   |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8742 µs            | 153.33 µs          | 3958%   |
| 930196       | FRONTIER        | 18               | 378,000    | 66.706 µs            | 293.96 µs          | 441%    |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 43.136 µs            | 197.79 µs          | 459%    |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 836.25 µs            | 4.6812 ms          | 560%    |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6901 ms            | 2.8226 ms          | 167%    |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 356.30 µs            | 1.3380 ms          | 376%    |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 107.46 µs            | 238.10 µs          | 222%    |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.154 µs            | 280.81 µs          | 322%    |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1813 ms            | 1.5543 ms          | 132%    |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 339.68 µs            | 363.10 µs          | 107%    |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 819.31 µs            | 1.5755 ms          | 192%    |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3500 ms            | 9.8543 ms          | 730%    |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8737 ms            | 1.0943 ms          | **58%** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3290 ms            | 681.83 µs          | **51%** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8394 ms            | 1.0994 ms          | **60%** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.563 µs            | 207.71 µs          | 275%    |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 869.18 µs            | 691.58 µs          | **80%** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2337 ms            | 17.237 ms          | 772%    |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3964 ms            | 1.7450 ms          | **73%** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.1240 ms            | 1.7190 ms          | **81%** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.0414 ms            | 22.338 ms          | 734%    |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.1039 ms            | 1.4159 ms          | **46%** |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.0778 ms            | 2.3369 ms          | **46%** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.7399 ms            | 44.936 ms          | 783%    |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.542 µs            | 171.92 µs          | 1490%   |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.0062 ms            | 4.5270 ms          | 226%    |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.7109 ms            | 7.4240 ms          | 200%    |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.6214 ms            | 844.53 µs          | **52%** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.3987 ms            | 1.4517 ms          | **43%** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.5592 ms            | 17.906 ms          | 322%    |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2905 ms            | 1.2935 ms          | **56%** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 416.82 µs            | 370.48 µs          | **89%** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 3.0149 ms            | 5.0561 ms          | 168%    |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
