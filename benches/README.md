# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6868 µs            | 5.6471 µs          | 1.53     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.742 µs            | 147.26 µs          | 2.27     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 94.591 µs            | 168.55 µs          | 1.78     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 853.45 µs            | 2.0192 ms          | 2.37     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6514 ms            | 2.0689 ms          | 1.25     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 355.14 µs            | 827.75 µs          | 2.33     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 141.04 µs            | 175.05 µs          | 1.24     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.02 µs            | 185.39 µs          | 1.49     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3249 ms            | 1.1832 ms          | **0.89** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 805.12 µs            | 552.95 µs          | **0.69** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6817 ms            | 5.7314 ms          | 2.14     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3236 ms            | 2.9968 ms          | 2.26     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1700 ms            | 1.8818 ms          | **0.45** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9764 ms            | 3.5463 ms          | **0.71** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5981 ms            | 3.0246 ms          | **0.84** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 797.86 µs            | 1.2374 ms          | 1.55     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4873 ms            | 5.1703 ms          | 1.15     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2942 ms            | 4.6659 ms          | 2.03     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2969 ms            | 6.0944 ms          | **0.97** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.687 ms            | 13.880 ms          | 1.3      |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3283 ms            | 7.6704 ms          | 1.77     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.423 ms            | 15.876 ms          | 1.28     |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.647 ms            | 14.502 ms          | **0.61** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7988 ms            | 10.445 ms          | 1.07     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.764 µs            | 26.065 µs          | 2.22     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0550 ms            | 4.5067 ms          | 1.48     |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2392 ms            | 5.4991 ms          | **0.6**  |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2030 ms            | 4.7621 ms          | **0.92** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.831 ms            | 17.188 ms          | 1.24     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.575 ms            | 11.876 ms          | **0.72** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.840 ms            | 15.524 ms          | 1.43     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2460 ms            | 2.3536 ms          | 1.05     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9112 ms            | 12.204 ms          | 1.23     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
