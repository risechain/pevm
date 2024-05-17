# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S   |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | ------- |
| 46147        | FRONTIER        | 1                | 21,000     | 4.0267 µs            | 6.4159 µs          | 159%    |
| 930196       | FRONTIER        | 18               | 378,000    | 67.678 µs            | 298.45 µs          | 441%    |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 43.994 µs            | 196.24 µs          | 446%    |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 849.96 µs            | 4.8613 ms          | 572%    |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6498 ms            | 2.8986 ms          | 176%    |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 357.89 µs            | 1.4183 ms          | 396%    |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 107.65 µs            | 239.41 µs          | 222%    |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 91.105 µs            | 279.88 µs          | 307%    |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1927 ms            | 1.5887 ms          | 133%    |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 334.68 µs            | 353.04 µs          | 105%    |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 824.72 µs            | 1.6190 ms          | 196%    |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3476 ms            | 10.124 ms          | 751%    |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8583 ms            | 1.1208 ms          | **60%** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3273 ms            | 684.92 µs          | **52%** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8199 ms            | 1.0897 ms          | **60%** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 75.205 µs            | 209.38 µs          | 278%    |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 865.63 µs            | 710.97 µs          | **82%** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2651 ms            | 17.649 ms          | 779%    |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3348 ms            | 1.7749 ms          | **76%** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.1125 ms            | 1.7354 ms          | **82%** |
| 12520364     | BERLIN          | 660              | 14,989,902 | 3.0102 ms            | 22.121 ms          | 735%    |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.0739 ms            | 1.4304 ms          | **47%** |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.0049 ms            | 2.3734 ms          | **47%** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.7978 ms            | 52.319 ms          | 902%    |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.501 µs            | 26.412 µs          | 230%    |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.0010 ms            | 4.5569 ms          | 228%    |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.6660 ms            | 7.6415 ms          | 208%    |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.6260 ms            | 844.23 µs          | **52%** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.3371 ms            | 1.4577 ms          | **44%** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.5110 ms            | 18.776 ms          | 341%    |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2674 ms            | 1.3047 ms          | **58%** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 417.60 µs            | 376.04 µs          | **90%** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9633 ms            | 5.1689 ms          | 174%    |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
