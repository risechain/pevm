# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7983 µs            | 5.5649 µs          | 1.47     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.700 µs            | 146.45 µs          | 2.26     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 93.393 µs            | 168.10 µs          | 1.8      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 862.84 µs            | 1.9862 ms          | 2.3      |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6656 ms            | 2.0655 ms          | 1.24     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 358.75 µs            | 820.41 µs          | 2.29     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 139.15 µs            | 175.92 µs          | 1.26     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 122.87 µs            | 185.95 µs          | 1.51     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3153 ms            | 1.1631 ms          | **0.88** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 800.83 µs            | 547.90 µs          | **0.68** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6875 ms            | 5.6822 ms          | 2.11     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3498 ms            | 2.9386 ms          | 2.18     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1645 ms            | 1.8790 ms          | **0.45** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8653 ms            | 3.4323 ms          | **0.71** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5926 ms            | 2.9023 ms          | **0.81** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 803.38 µs            | 1.2212 ms          | 1.52     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4898 ms            | 5.1056 ms          | 1.14     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.3054 ms            | 4.5902 ms          | 1.99     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2863 ms            | 5.9966 ms          | **0.95** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.648 ms            | 13.655 ms          | 1.28     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2810 ms            | 7.5984 ms          | 1.77     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.292 ms            | 15.903 ms          | 1.29     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.725 ms            | 14.307 ms          | **0.6**  |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7585 ms            | 10.382 ms          | 1.06     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.617 µs            | 26.134 µs          | 2.25     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1136 ms            | 4.4381 ms          | 1.43     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.1965 ms            | 5.4584 ms          | **0.59** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2213 ms            | 4.7447 ms          | **0.91** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.778 ms            | 16.983 ms          | 1.23     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.699 ms            | 11.750 ms          | **0.7**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.752 ms            | 15.342 ms          | 1.43     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2223 ms            | 2.3735 ms          | 1.07     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8799 ms            | 12.195 ms          | 1.23     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
