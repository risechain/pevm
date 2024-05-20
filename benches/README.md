# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7454 µs            | 5.5960 µs          | 1.49     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.292 µs            | 286.70 µs          | 4.53     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 42.858 µs            | 194.04 µs          | 4.53     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 800.81 µs            | 2.6976 ms          | 3.37     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6871 ms            | 2.8224 ms          | 1.67     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 342.77 µs            | 891.17 µs          | 2.6      |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 104.76 µs            | 231.22 µs          | 2.21     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.888 µs            | 278.55 µs          | 3.17     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1425 ms            | 1.2132 ms          | 1.06     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 338.81 µs            | 338.98 µs          | 1        |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 810.08 µs            | 1.5599 ms          | 1.93     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.2779 ms            | 4.5065 ms          | 3.53     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8560 ms            | 1.1395 ms          | **0.61** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3109 ms            | 666.54 µs          | **0.51** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.7901 ms            | 927.65 µs          | **0.52** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.212 µs            | 204.77 µs          | 2.72     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 865.63 µs            | 687.66 µs          | **0.79** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.1912 ms            | 8.6804 ms          | 3.96     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3233 ms            | 1.7566 ms          | **0.76** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.0714 ms            | 1.7226 ms          | **0.83** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.9318 ms            | 11.362 ms          | 3.88     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.0618 ms            | 1.4104 ms          | **0.46** |
| 12965000     | LONDON          | 259              | 30,025,257 | 4.9950 ms            | 2.3574 ms          | **0.47** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.6388 ms            | 23.864 ms          | 4.23     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.486 µs            | 25.911 µs          | 2.26     |
| 15537394     | MERGE           | 80               | 29,983,006 | 1.9781 ms            | 4.5800 ms          | 2.32     |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.5679 ms            | 6.0078 ms          | 1.68     |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.5939 ms            | 816.94 µs          | **0.51** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.2988 ms            | 1.4214 ms          | **0.43** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.3557 ms            | 11.136 ms          | 2.08     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2229 ms            | 1.2543 ms          | **0.56** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 410.50 µs            | 361.50 µs          | **0.88** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9102 ms            | 4.2529 ms          | 1.46     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
