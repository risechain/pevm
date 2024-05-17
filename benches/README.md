# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7893 µs            | 5.6218 µs          | 148%     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.106 µs            | 292.39 µs          | 456%     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 42.700 µs            | 193.29 µs          | 453%     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 805.89 µs            | 2.9533 ms          | 366%     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6458 ms            | 2.7679 ms          | 168%     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 351.72 µs            | 951.92 µs ms       | 271%     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 104.34 µs            | 237.98 µs          | 228%     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 87.465 µs            | 277.47 µs          | 317%     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.1451 ms            | 1.5329 ms          | 134%     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 336.60 µs            | 336.37 µs          | **100%** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 800.32 µs            | 1.5738 ms          | 197%     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.2828 ms            | 5.1713 ms          | 403%     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 1.8678 ms            | 1.0774 ms          | **58%**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 1.3168 ms            | 677.30 µs          | **51%**  |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 1.8126 ms            | 1.0952 ms          | **60%**  |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 76.211 µs            | 207.81 µs          | 273%     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 869.43 µs            | 687.94 µs          | **79%**  |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.1546 ms            | 10.628 ms          | 493%     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 2.3170 ms            | 1.7440 ms          | **75%**  |
| 12244000     | BERLIN          | 133              | 12,450,737 | 2.0852 ms            | 1.7000 ms          | **82%**  |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.8972 ms            | 13.155 ms          | 454%     |
| 12964999     | BERLIN          | 145              | 15,026,712 | 3.0749 ms            | 1.4112 ms          | **46%**  |
| 12965000     | LONDON          | 259              | 30,025,257 | 5.0060 ms            | 2.3347 ms          | **47%**  |
| 13217637     | LONDON          | 1100             | 29,985,362 | 5.5879 ms            | 27.722 ms          | 496%     |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.361 µs            | 25.746 µs          | 227%     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.0382 ms            | 4.7525 ms          | 233%     |
| 16146267     | MERGE           | 473              | 19,204,593 | 3.5933 ms            | 6.7612 ms          | 188%     |
| 17034869     | MERGE           | 93               | 8,450,250  | 1.6057 ms            | 839.54 µs          | **52%**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 3.3138 ms            | 1.4299 ms          | **43%**  |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 5.3944 ms            | 12.337 ms          | 229%     |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 2.2429 ms            | 1.2538 ms          | **56%**  |
| 19426587     | CANCUN          | 37               | 2,633,933  | 414.28 µs            | 372.63 µs          | **90%**  |
| 19638737     | CANCUN          | 381              | 15,932,416 | 2.9314 ms            | 4.7400 ms          | 162%     |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
