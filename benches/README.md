# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.7875 µs            | 5.5462 µs          | 1.46     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.442 µs            | 128.19 µs          | 1.99     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 93.398 µs            | 118.05 µs          | 1.26     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 823.50 µs            | 1.5025 ms          | 1.82     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6456 ms            | 1.8843 ms          | 1.15     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 351.05 µs            | 626.17 µs          | 1.78     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 135.63 µs            | 120.48 µs          | **0.89** |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 120.46 µs            | 126.91 µs          | 1.05     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3051 ms            | 666.98 µs          | **0.51** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 782.92 µs            | 385.22 µs          | **0.49** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6935 ms            | 2.4376 ms          | **0.9**  |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.2934 ms            | 2.4974 ms          | 1.93     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1591 ms            | 1.3473 ms          | **0.32** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8165 ms            | 2.4244 ms          | **0.5**  |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5850 ms            | 1.4446 ms          | **0.4**  |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 779.53 µs            | 950.85 µs          | 1.22     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4246 ms            | 2.8749 ms          | **0.65** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2213 ms            | 4.0127 ms          | 1.81     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2078 ms            | 3.1334 ms          | **0.5**  |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.586 ms            | 8.1556 ms          | **0.77** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0983 ms            | 4.4223 ms          | 1.43     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.1844 ms            | 5.6365 ms          | 1.35     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0921 ms            | 2.0454 ms          | **0.5**  |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.186 ms            | 10.046 ms          | **0.82** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.731 ms            | 8.0878 ms          | **0.34** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.7101 ms            | 7.8470 ms          | **0.81** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5030 ms            | 10.139 ms          | 1.84     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.051 ms            | 2.9510 ms          | **0.29** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.434 ms            | 6.7025 ms          | **0.5**  |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.387 ms            | 6.2002 ms          | **0.43** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.3821 ms            | 9.3776 ms          | 1.47     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.735 ms            | 5.1089 ms          | **0.48** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.618 µs            | 13.185 µs          | 1.13     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9862 ms            | 1.9273 ms          | **0.65** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.127 ms            | 7.7034 ms          | **0.59** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.0455 ms            | 3.3676 ms          | **0.37** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1494 ms            | 2.5625 ms          | **0.5**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.655 ms            | 9.0167 ms          | **0.66** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.267 ms            | 8.2498 ms          | **0.51** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.669 ms            | 8.7295 ms          | **0.82** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2105 ms            | 1.3707 ms          | **0.62** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8026 ms            | 6.5636 ms          | **0.67** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.199 ms            | 10.172 ms          | **0.48** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.5394 ms            | 4.3923 ms          | **0.51** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2860 ms            | 1.0768 ms          | **0.84** |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0206 ms            | 2.9096 ms          | **0.58** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.137 ms            | 6.1068 ms          | **0.6**  |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.893 ms            | 6.9759 ms          | **0.59** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.830 ms            | 7.8959 ms          | **0.62** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 913.88 µs            | 619.21 µs          | **0.68** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1361 ms            | 3.9636 ms          | **0.65** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.153 ms            | 4.0223 ms          | **0.4**  |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3072 ms            | 1.4186 ms          | **0.61** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S    |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | -------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 194.37 ms            | 153.66 ms          | **0.79** |
