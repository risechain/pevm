# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6766 µs            | 5.5740 µs          | 1.52     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.423 µs            | 138.54 µs          | 2.18     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 92.547 µs            | 165.06 µs          | 1.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 842.56 µs            | 1.8589 ms          | 2.21     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6543 ms            | 2.0144 ms          | 1.22     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 358.38 µs            | 775.96 µs          | 2.17     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 140.89 µs            | 172.97 µs          | 1.23     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 122.65 µs            | 179.45 µs          | 1.46     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3139 ms            | 1.0404 ms          | **0.79** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 790.27 µs            | 535.04 µs          | **0.68** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6794 ms            | 5.5789 ms          | 2.08     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3398 ms            | 2.7562 ms          | 2.06     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1325 ms            | 1.8857 ms          | **0.46** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8355 ms            | 3.3167 ms          | **0.69** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6019 ms            | 2.7731 ms          | **0.77** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 795.41 µs            | 1.2223 ms          | 1.54     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4209 ms            | 5.1238 ms          | 1.16     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2303 ms            | 4.4384 ms          | 1.99     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2440 ms            | 5.9882 ms          | **0.96** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.636 ms            | 13.572 ms          | 1.28     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2360 ms            | 7.5423 ms          | 1.78     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.254 ms            | 15.639 ms          | 1.28     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.495 ms            | 14.322 ms          | **0.61** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7661 ms            | 9.4475 ms          | **0.97** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.714 µs            | 25.765 µs          | 2.2      |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0485 ms            | 3.1299 ms          | 1.03     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.1762 ms            | 5.3246 ms          | **0.58** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1910 ms            | 4.6702 ms          | **0.9**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.842 ms            | 16.740 ms          | 1.21     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.637 ms            | 11.654 ms          | **0.7**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.716 ms            | 15.273 ms          | 1.43     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2341 ms            | 2.3952 ms          | 1.07     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8716 ms            | 12.113 ms          | 1.23     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
