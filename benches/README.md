# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7122 µs            | 5.6496 µs          | 1.52     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.142 µs            | 139.69 µs          | 2.18     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 95.198 µs            | 150.70 µs          | 1.58     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 819.03 µs            | 1.7548 ms          | 2.14     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6334 ms            | 1.8923 ms          | 1.16     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 349.50 µs            | 749.26 µs          | 2.14     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 138.08 µs            | 159.20 µs          | 1.15     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 125.79 µs            | 157.83 µs          | 1.25     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3175 ms            | 816.83 µs          | **0.62** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 797.93 µs            | 421.35 µs          | **0.53** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7912 ms            | 4.2618 ms          | 1.53     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3248 ms            | 3.0461 ms          | 2.3      |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2267 ms            | 1.7088 ms          | **0.4**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9564 ms            | 2.7736 ms          | **0.56** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6625 ms            | 1.9413 ms          | **0.53** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 792.84 µs            | 999.19 µs          | 1.26     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6988 ms            | 4.2741 ms          | **0.91** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2856 ms            | 4.6759 ms          | 2.05     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.4401 ms            | 4.9273 ms          | **0.77** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.224 ms            | 11.158 ms          | **0.99** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0374 ms            | 5.1470 ms          | 1.69     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3812 ms            | 6.6665 ms          | 1.52     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.1916 ms            | 3.2201 ms          | **0.77** |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.756 ms            | 12.520 ms          | **0.98** |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.252 ms            | 11.454 ms          | **0.51** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.9047 ms            | 9.2852 ms          | **0.94** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.8069 ms            | 13.312 ms          | 2.29     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.390 ms            | 4.2383 ms          | **0.41** |
| 14334629     | LONDON          | 819              | 30,135,754 | 14.050 ms            | 9.3966 ms          | **0.67** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.841 ms            | 8.6506 ms          | **0.58** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4076 ms            | 10.671 ms          | 1.67     |
| 15199017     | LONDON          | 866              | 30,028,395 | 11.061 ms            | 6.1022 ms          | **0.55** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.364 µs            | 25.983 µs          | 2.29     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1494 ms            | 2.6637 ms          | **0.85** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.696 ms            | 10.320 ms          | **0.75** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.4603 ms            | 4.5001 ms          | **0.48** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.4792 ms            | 3.6824 ms          | **0.67** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.360 ms            | 13.244 ms          | **0.92** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.954 ms            | 12.179 ms          | **0.72** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.237 ms            | 13.478 ms          | 1.2      |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2529 ms            | 1.5025 ms          | **0.67** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.424 ms            | 9.9828 ms          | **0.96** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 22.078 ms            | 16.489 ms          | **0.75** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.9507 ms            | 6.8987 ms          | **0.77** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3539 ms            | 1.5265 ms          | 1.13     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.2012 ms            | 4.5476 ms          | **0.87** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.850 ms            | 8.9319 ms          | **0.82** |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.418 ms            | 7.8153 ms          | **0.63** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.511 ms            | 11.792 ms          | **0.87** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 949.75 µs            | 855.58 µs          | **0.9**  |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.5148 ms            | 5.9448 ms          | **0.91** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.685 ms            | 5.7162 ms          | **0.53** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4768 ms            | 1.9065 ms          | **0.77** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
