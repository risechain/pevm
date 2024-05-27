# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7708 µs            | 5.3011 µs          | 1.41     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.784 µs            | 126.81 µs          | 1.99     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.011 µs            | 141.32 µs          | 1.55     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 822.09 µs            | 1.5732 ms          | 1.91     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6773 ms            | 1.9072 ms          | 1.14     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 349.16 µs            | 670.33 µs          | 1.92     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 137.57 µs            | 148.18 µs          | 1.08     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 120.92 µs            | 145.11 µs          | 1.2      |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3251 ms            | 722.60 µs          | **0.55** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 781.11 µs            | 386.37 µs          | **0.49** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7367 ms            | 3.8830 ms          | 1.42     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3276 ms            | 2.6306 ms          | 1.98     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1593 ms            | 1.6159 ms          | **0.39** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8660 ms            | 2.6964 ms          | **0.55** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5924 ms            | 1.9085 ms          | **0.53** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 781.99 µs            | 983.79 µs          | 1.26     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5383 ms            | 4.0281 ms          | **0.89** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2646 ms            | 4.0547 ms          | 1.79     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3024 ms            | 4.5665 ms          | **0.72** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.962 ms            | 10.553 ms          | **0.96** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0840 ms            | 4.6802 ms          | 1.52     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3286 ms            | 6.1620 ms          | 1.42     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0955 ms            | 2.9602 ms          | **0.72** |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.478 ms            | 11.795 ms          | **0.95** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.887 ms            | 10.966 ms          | **0.46** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8652 ms            | 8.2621 ms          | **0.84** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.6181 ms            | 10.433 ms          | 1.86     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.208 ms            | 4.1804 ms          | **0.41** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.749 ms            | 9.3324 ms          | **0.68** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.560 ms            | 8.0214 ms          | **0.55** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4616 ms            | 9.6931 ms          | 1.5      |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.903 ms            | 5.6515 ms          | **0.52** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.302 µs            | 23.862 µs          | 2.11     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0580 ms            | 2.5210 ms          | **0.82** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.502 ms            | 9.4085 ms          | **0.7**  |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.2554 ms            | 4.2455 ms          | **0.46** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3108 ms            | 3.4269 ms          | **0.65** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.083 ms            | 12.447 ms          | **0.88** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.590 ms            | 11.403 ms          | **0.69** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.944 ms            | 12.615 ms          | 1.15     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2179 ms            | 1.4594 ms          | **0.66** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.140 ms            | 9.1996 ms          | **0.91** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.671 ms            | 16.981 ms          | **0.78** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.7210 ms            | 6.4054 ms          | **0.73** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3192 ms            | 1.4411 ms          | 1.09     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1056 ms            | 4.1899 ms          | **0.82** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.471 ms            | 8.5175 ms          | **0.81** |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.099 ms            | 7.5640 ms          | **0.63** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.186 ms            | 11.060 ms          | **0.84** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 932.28 µs            | 800.30 µs          | **0.86** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.3033 ms            | 5.4695 ms          | **0.87** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.409 ms            | 5.3239 ms          | **0.51** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4080 ms            | 1.7398 ms          | **0.72** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
