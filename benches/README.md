# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8273 µs            | 5.4549 µs          | 1.43     |
| 930196       | FRONTIER        | 18               | 378,000    | 65.028 µs            | 128.84 µs          | 1.98     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 91.971 µs            | 144.06 µs          | 1.57     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 844.26 µs            | 1.5933 ms          | 1.89     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6694 ms            | 1.9006 ms          | 1.14     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 351.52 µs            | 680.40 µs          | 1.94     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 139.51 µs            | 151.21 µs          | 1.08     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 127.42 µs            | 146.53 µs          | 1.15     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3182 ms            | 737.83 µs          | **0.56** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 790.10 µs            | 386.09 µs          | **0.48** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7951 ms            | 3.9428 ms          | 1.41     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3549 ms            | 2.6894 ms          | 1.98     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2517 ms            | 1.6428 ms          | **0.39** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9548 ms            | 2.7234 ms          | **0.55** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6039 ms            | 1.7388 ms          | **0.48** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 774.78 µs            | 969.61 µs          | 1.25     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6350 ms            | 4.0882 ms          | **0.88** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2967 ms            | 4.1505 ms          | 1.81     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.4534 ms            | 4.6074 ms          | **0.71** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.279 ms            | 10.765 ms          | **0.95** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.1077 ms            | 4.7574 ms          | 1.53     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3194 ms            | 6.2069 ms          | 1.44     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.2215 ms            | 2.9701 ms          | **0.7**  |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.768 ms            | 11.881 ms          | **0.93** |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.429 ms            | 10.790 ms          | **0.44** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8710 ms            | 8.3445 ms          | **0.85** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5718 ms            | 11.134 ms          | 2        |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.331 ms            | 3.9401 ms          | **0.38** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.969 ms            | 9.0942 ms          | **0.65** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.829 ms            | 8.2517 ms          | **0.56** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.5447 ms            | 9.6252 ms          | 1.47     |
| 15199017     | LONDON          | 866              | 30,028,395 | 11.060 ms            | 5.7029 ms          | **0.52** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.356 µs            | 23.293 µs          | 2.05     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1138 ms            | 2.5644 ms          | **0.82** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.693 ms            | 9.4309 ms          | **0.69** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.4145 ms            | 4.0575 ms          | **0.43** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.4308 ms            | 3.4895 ms          | **0.64** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.376 ms            | 12.536 ms          | **0.87** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.750 ms            | 11.517 ms          | **0.69** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.171 ms            | 12.962 ms          | 1.16     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2375 ms            | 1.4649 ms          | **0.65** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.383 ms            | 9.3176 ms          | **0.9**  |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.855 ms            | 14.669 ms          | **0.67** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.9795 ms            | 6.5022 ms          | **0.72** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3450 ms            | 1.4666 ms          | 1.09     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1882 ms            | 4.2726 ms          | **0.82** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.819 ms            | 8.5780 ms          | **0.79** |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.258 ms            | 7.5834 ms          | **0.62** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.469 ms            | 11.205 ms          | **0.83** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 944.83 µs            | 811.20 µs          | **0.86** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.4719 ms            | 5.5570 ms          | **0.86** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.686 ms            | 5.4156 ms          | **0.51** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4851 ms            | 1.8183 ms          | **0.73** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
