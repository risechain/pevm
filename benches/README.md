# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.6706 µs            | 5.6144 µs          | 1.53     |
| 930196       | FRONTIER        | 18               | 378,000    | 63.543 µs            | 136.19 µs          | 2.14     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 92.899 µs            | 149.75 µs          | 1.61     |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 842.28 µs            | 1.6945 ms          | 2.01     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6189 ms            | 1.8890 ms          | 1.17     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 352.70 µs            | 719.25 µs          | 2.04     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 139.53 µs            | 155.07 µs          | 1.11     |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 124.12 µs            | 154.12 µs          | 1.24     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3387 ms            | 782.29 ms          | **0.58** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 800.01 µs            | 420.78 µs          | **0.53** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.7857 ms            | 4.2709 ms          | 1.53     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3094 ms            | 2.8677 ms          | 2.19     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.2208 ms            | 1.6949 ms          | **0.4**  |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.9368 ms            | 2.7956 ms          | **0.57** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.6284 ms            | 1.9152 ms          | **0.53** |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 794.59 µs            | 1.0077 µs          | 1.27     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.6120 ms            | 4.2585 ms          | **0.92** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2202 ms            | 4.4828 ms          | 2.02     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.3850 ms            | 4.9007 ms          | **0.77** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 11.112 ms            | 11.130 ms          | 1        |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.1003 ms            | 4.9854 ms          | 1.61     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.3341 ms            | 6.4962 ms          | 1.5      |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.1729 ms            | 3.2003 ms          | **0.77** |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.636 ms            | 12.350 ms          | **0.98** |
| 12965000     | LONDON          | 259              | 30,025,257 | 24.131 ms            | 11.280 ms          | **0.47** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.8948 ms            | 9.0780 ms          | **0.92** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.6204 ms            | 12.764 ms          | 2.27     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.310 ms            | 4.1837 ms          | **0.41** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.887 ms            | 9.3024 ms          | **0.67** |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.797 ms            | 8.4609 ms          | **0.57** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.3996 ms            | 10.363 ms          | 1.62     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.987 ms            | 5.9905 µs          | **0.55** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.666 µs            | 26.044 µs          | 2.23     |
| 15537394     | MERGE           | 80               | 29,983,006 | 3.1671 ms            | 2.6375 ms          | **0.83** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.593 ms            | 10.093 ms          | **0.74** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.4999 ms            | 4.3734 ms          | **0.46** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.3712 ms            | 3.6455 ms          | **0.68** |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 14.284 ms            | 13.195 ms          | **0.92** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.981 ms            | 12.010 ms          | **0.71** |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 11.071 ms            | 13.398 ms          | 1.21     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2588 ms            | 1.5037 ms          | **0.67** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 10.189 ms            | 9.8842 ms          | **0.97** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.903 ms            | 16.617 ms          | **0.76** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.8075 ms            | 6.8411 ms          | **0.78** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.3314 ms            | 1.5117 ms          | 1.14     |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.1411 ms            | 4.5174 ms          | **0.88** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.671 ms            | 8.9348 ms          | **0.84** |
| 19932703     | CANCUN          | 143              | 10,421,765 | 12.311 ms            | 7.7787 ms          | **0.63** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 13.359 ms            | 11.729 ms          | **0.88** |
| 19933122     | CANCUN          | 45               | 2,056,821  | 946.43 µs            | 848.76 µs          | **0.9**  |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.4247 ms            | 5.8688 ms          | **0.91** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.529 ms            | 5.6784 ms          | **0.54** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.4376 ms            | 1.7968 ms          | **0.74** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks in the future. All blocks are currently in the CANCUN spec.

|                           | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S   |
| ------------------------- | ---------------- | ------------- | -------------------- | ------------------ | ------- |
| Independent Raw Transfers | 47,620           | 1,000,020,000 | 197.22 ms            | 178.64 ms          | **91%** |
