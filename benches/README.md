# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7527 µs            | 5.6287 µs          | 1.5      |
| 930196       | FRONTIER        | 18               | 378,000    | 65.817 µs            | 140.55 µs          | 2.14     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 90.237 µs            | 144.24 µs          | 1.6      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 837.44 µs            | 1.7299 ms          | 2.07     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6688 ms            | 1.9127 ms          | 1.15     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 352.61 µs            | 735.70 µs          | 2.09     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 136.35 µs            | 148.85 µs          | 1.09     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 121.94 µs            | 152.12 µs          | 1.25     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3213 ms            | 757.08 µs          | **0.57** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 775.90 µs            | 405.51 µs          | **0.52** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7141 ms            | 3.9182 ms          | 1.44     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3202 ms            | 2.9410 ms          | 2.23     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1489 ms            | 1.6120 ms          | **0.39** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8524 ms            | 2.6929 ms          | **0.55** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6296 ms            | 1.8315 ms          | **0.5**  |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 780.34 µs            | 997.00 µs          | 1.28     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5124 ms            | 4.0183 ms          | **0.89** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2674 ms            | 4.5731 ms          | 2.02     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3087 ms            | 4.5717 ms          | **0.72** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.850 ms            | 10.507 ms          | **0.97** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.1399 ms            | 5.0352 ms          | 1.6      |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3215 ms            | 6.4919 ms          | 1.5      |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0694 ms            | 2.9552 ms          | **0.73** |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.359 ms            | 11.740 ms          | **0.95** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.727 ms            | 10.629 ms          | **0.45** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8725 ms            | 8.9664 ms          | **0.91** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.7870 ms            | 12.915 ms          | 2.23     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.181 ms            | 3.9407 ms          | **0.39** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.680 ms            | 8.8924 ms          | **0.65** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.512 ms            | 8.0746 ms          | **0.56** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.5255 ms            | 10.270 ms          | 1.57     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.878 ms            | 5.8955 ms          | **0.54** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.318 µs            | 23.306 µs          | 2.06     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0803 ms            | 2.5144 ms          | **0.82** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.432 ms            | 9.8760 ms          | **0.74** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2180 ms            | 4.1579 ms          | **0.45** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.2769 ms            | 3.4466 ms          | **0.65** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.980 ms            | 12.440 ms          | **0.89** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.704 ms            | 11.302 ms          | **0.68** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.882 ms            | 12.580 ms          | 1.16     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2193 ms            | 1.4731 ms          | **0.66** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.084 ms            | 9.3425 ms          | **0.93** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.566 ms            | 15.462 ms          | **0.72** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.6788 ms            | 6.4171 ms          | **0.74** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3079 ms            | 1.4366 ms          | 1.1      |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0708 ms            | 4.2019 ms          | **0.83** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.535 ms            | 8.4413 ms          | **0.8**  |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.119 ms            | 7.5509 ms          | **0.62** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.183 ms            | 11.013 ms          | **0.84** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 933.05 µs            | 801.25 µs          | **0.86** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.2860 ms            | 5.4989 ms          | **0.87** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.343 ms            | 5.3371 ms          | **0.52** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4003 ms            | 1.7399 ms          | **0.72** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
