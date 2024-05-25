# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8912 µs            | 5.5717 µs          | 1.43     |
| 930196       | FRONTIER        | 18               | 378,000    | 67.616 µs            | 135.03 µs          | 2        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 94.619 µs            | 149.28 µs          | 1.58     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 865.77 µs            | 1.7026 ms          | 1.97     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.7112 ms            | 1.9037 ms          | 1.11     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 359.85 µs            | 725.39 µs          | 2.02     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 139.31 µs            | 154.12 µs          | 1.11     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.77 µs            | 155.23 µs          | 1.24     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3216 ms            | 760.24 ms          | **0.58** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 802.49 µs            | 422.65 µs          | **0.53** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7445 ms            | 4.1716 ms          | 1.52     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.4077 ms            | 2.8973 ms          | 2.06     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1686 ms            | 1.6511 ms          | **0.4**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8736 ms            | 2.7334 ms          | **0.56** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6110 ms            | 1.9057 ms          | **0.53** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 804.16 µs            | 1.0061 µs          | 1.25     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4964 ms            | 4.1332 ms          | **0.92** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2906 ms            | 4.5204 ms          | 1.97     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3538 ms            | 4.8390 ms          | **0.76** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.736 ms            | 10.743 ms          | 1        |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.1467 ms            | 5.0001 ms          | 1.59     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3120 ms            | 6.4414 ms          | 1.49     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.1449 ms            | 3.1534 ms          | **0.76** |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.365 ms            | 12.072 ms          | **0.98** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.681 ms            | 11.040 ms          | **0.47** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8508 ms            | 9.0332 ms          | **0.92** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.7925 ms            | 12.937 ms          | 2.23     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.284 ms            | 4.1363 ms          | **0.4**  |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.651 ms            | 8.8894 ms          | **0.65** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.524 ms            | 8.2944 ms          | **0.57** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4874 ms            | 10.378 ms          | 1.6      |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.880 ms            | 5.9462 µs          | **0.55** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.744 µs            | 25.956 µs          | 2.21     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0872 ms            | 2.5540 ms          | **0.83** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.397 ms            | 10.193 ms          | **0.76** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.3233 ms            | 4.2923 ms          | **0.46** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2633 ms            | 3.5575 ms          | **0.68** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.862 ms            | 12.888 ms          | **0.93** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.763 ms            | 11.714 ms          | **0.7**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.821 ms            | 13.055 ms          | 1.21     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2310 ms            | 1.4987 ms          | **0.67** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.9714 ms            | 9.6187 ms          | **0.96** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.592 ms            | 16.480 ms          | **0.76** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.5751 ms            | 6.6900 ms          | **0.78** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2884 ms            | 1.4653 ms          | 1.14     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1064 ms            | 4.4120 ms          | **0.86** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.340 ms            | 8.6654 ms          | **0.84** |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.162 ms            | 7.7181 ms          | **0.63** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.081 ms            | 11.436 ms          | **0.87** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 938.76 µs            | 834.02 µs          | **0.89** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.3301 ms            | 5.7545 ms          | **0.91** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.268 ms            | 5.6551 ms          | **0.55** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3479 ms            | 1.7149 ms          | **0.73** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
