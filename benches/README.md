# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6562 µs            | 5.3819 µs          | 1.47     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.025 µs            | 126.95 µs          | 2.01     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.022 µs            | 119.68 µs          | 1.31     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 816.82 µs            | 1.5073 ms          | 1.85     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6552 ms            | 1.8821 ms          | 1.14     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 357.25 µs            | 627.51 µs          | 1.76     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 133.45 µs            | 121.17 µs          | **0.91** |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 120.15 µs            | 126.62 µs          | 1.05     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3105 ms            | 674.77 µs          | **0.51** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 781.22 µs            | 385.97 µs          | **0.49** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7177 ms            | 2.5159 ms          | **0.93** |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3196 ms            | 2.4965 ms          | 1.89     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1775 ms            | 1.3807 ms          | **0.33** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8917 ms            | 2.4407 ms          | **0.5**  |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5997 ms            | 1.4140 ms          | **0.39** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 765.40 µs            | 935.89 µs          | 1.22     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.5708 ms            | 2.9904 ms          | **0.65** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2654 ms            | 4.0128 ms          | 1.77     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3308 ms            | 3.2053 ms          | **0.51** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.022 ms            | 8.5240 ms          | **0.77** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0846 ms            | 4.4592 ms          | 1.45     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3446 ms            | 5.7403 ms          | 1.32     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.1535 ms            | 2.0954 ms          | **0.5**  |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.537 ms            | 10.452 ms          | **0.83** |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.032 ms            | 8.2775 ms          | **0.34** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.9000 ms            | 7.9108 ms          | **0.8**  |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5950 ms            | 10.221 ms          | 1.83     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.247 ms            | 2.9894 ms          | **0.29** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.858 ms            | 6.8733 ms          | **0.5**  |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.714 ms            | 6.2710 ms          | **0.43** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.4588 ms            | 9.4312 ms          | 1.46     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.959 ms            | 5.1888 ms          | **0.47** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.297 µs            | 12.998 µs          | 1.15     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.0862 ms            | 2.0264 ms          | **0.66** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.723 ms            | 7.8384 ms          | **0.57** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.3351 ms            | 3.3856 ms          | **0.36** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.4014 ms            | 2.7044 ms          | **0.5**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.286 ms            | 9.3381 ms          | **0.65** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.624 ms            | 8.6692 ms          | **0.52** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.083 ms            | 9.1971 ms          | **0.83** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2388 ms            | 1.3834 ms          | **0.62** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.256 ms            | 6.9582 ms          | **0.68** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.754 ms            | 10.324 ms          | **0.47** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.8818 ms            | 4.6027 ms          | **0.52** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3435 ms            | 1.1450 ms          | **0.85** |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1268 ms            | 3.0098 ms          | **0.59** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.716 ms            | 6.3824 ms          | **0.6**  |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.195 ms            | 7.0225 ms          | **0.58** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.359 ms            | 8.0875 ms          | **0.61** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 935.87 µs            | 635.97 µs          | **0.68** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.4159 ms            | 4.1064 ms          | **0.64** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.560 ms            | 4.1813 ms          | **0.4**  |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4634 ms            | 1.5497 ms          | **0.63** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
