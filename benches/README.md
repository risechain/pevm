# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7872 µs            | 5.5558 µs          | 1.47     |
| 930196       | FRONTIER        | 18               | 378,000    | 65.824 µs            | 140.22 µs          | 2.13     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 95.617 µs            | 156.59 µs          | 1.64     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 845.84 µs            | 1.7590 ms          | 2.08     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6563 ms            | 1.9527 ms          | 1.18     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 349.50 µs            | 774.49 µs          | 2.22     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 143.31 µs            | 164.90 µs          | 1.15     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.42 µs            | 163.38 µs          | 1.31     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3205 ms            | 884.04 ms          | **0.67** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 799.71 µs            | 494.42 µs          | **0.62** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7349 ms            | 4.3211 ms          | 1.58     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3382 ms            | 2.9384 ms          | 2.2      |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2040 ms            | 1.7674 ms          | **0.42** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9054 ms            | 2.8604 ms          | **0.58** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6187 ms            | 2.1291 ms          | **0.59** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 789.76 µs            | 1.0295 ms          | 1.3      |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4798 ms            | 4.2522 ms          | **0.95** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.3119 ms            | 4.6240 ms          | 2        |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3067 ms            | 5.0388 ms          | **0.8**  |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.857 ms            | 11.021 ms          | 1.02     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2420 ms            | 6.6671 ms          | 1.57     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.578 ms            | 12.488 ms          | **0.99** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.900 ms            | 11.517 ms          | **0.48** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7959 ms            | 9.3456 ms          | **0.95** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.533 µs            | 25.709 µs          | 2.23     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0648 ms            | 3.1080 ms          | 1.01     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2805 ms            | 4.8756 ms          | **0.53** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2796 ms            | 3.6964 ms          | **0.7**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.876 ms            | 13.367 ms          | **0.96** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.627 ms            | 12.187 ms          | **0.73** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.824 ms            | 13.264 ms          | 1.23     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2348 ms            | 1.5202 ms          | **0.68** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9403 ms            | 9.9324 ms          | **1**    |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
