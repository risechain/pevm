# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block.

All state data needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S   |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ------- |
| 46147        | FRONTIER        | 1                | 21,000     | 4.0228 µs            | 228.85 µs          | 5689%   |
| 930196       | FRONTIER        | 18               | 378,000    | 66.342 µs            | 591.65 µs          | 892%    |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 47.980 µs            | 285.17 µs          | 594%    |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 938.30 µs            | 6.1501 ms          | 655%    |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.7243 ms            | 4.3153 ms          | 250%    |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 422.14 µs            | 1.9695 ms          | 467%    |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 134.96 µs            | 325.36 µs          | 241%    |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 117.41 µs            | 382.38 µs          | 326%    |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.2494 ms            | 3.2173 ms          | 258%    |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 1.2494 ms            | 3.2173 ms          | 128%    |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 895.79 µs            | 2.1611 ms          | 241%    |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3612 ms            | 18.635 ms          | 1369%   |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.9422 ms            | 1.8156 ms          | **93%** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.4630 ms            | 1.0538 ms          | **72%** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.9942 ms            | 1.2132 ms          | **61%** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.777 µs            | 290.25 µs          | 383%    |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 910.91 µs            | 935.13 µs          | 103%    |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.3492 ms            | 30.097 ms          | 1281%   |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.5519 ms            | 6.1147 ms          | 240%    |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.2908 ms            | 1.0627 ms          | **46%** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.1306 ms            | 37.337 ms          | 1193%   |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.3104 ms            | 2.2533 ms          | **68%** |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.4522 ms            | 2.8923 ms          | **53%** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.9838 ms            | 6.3092 s           | 105438% |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.594 µs            | 251.24 µs          | 2167%   |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.0481 ms            | 5.5513 ms          | 271%    |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.8539 ms            | 153.77 ms          | 3990%   |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.7898 ms            | 1.0811 ms          | **60%** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.6705 ms            | 2.1705 ms          | **59%** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.7643 ms            | 874.75 ms          | 15175%  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.5718 ms            | 1.2886 ms          | **50%** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 440.34 µs            | 493.62 µs          | 112%    |
| 19638737     | CANCUN          | 381              | 15,932,416 | 3.1573 ms            | 57.751 ms          | 1829%   |
