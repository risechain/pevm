# Benchmarks

Current benchmarks are run on a Linux machine with an Intel i9-12900K (24 CPUs @5.20 GHz) and 32 GB RAM. Future benchmarks will be run on more standard Cloud services that many tend to run nodes on.

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to bench 100 samples for each sequential & parallel execution of a block. The hardcoded concurrency level is 16, and all state needed is currently loaded into memory before execution.

## Ethereum Mainnet Blocks

This benchmark includes a few transactions for each Ethereum hardfork that changes the EVM spec. While we mainly bench large blocks for potentially significant parallelism gains, we include a few small ones to ensure we also handle the edge cases well. For instance, the parallel overheads are usually not worth it for small blocks, which we expect the PEVM to fall back to and match sequential execution's performance (TODO).

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential Execution | Parallel Execution | P / S    |
| ------------ | --------------- | ---------------- | ---------- | -------------------- | ------------------ | -------- |
| 46147        | FRONTIER        | 1                | 21,000     | 3.8133 µs            | 5.5464 µs          | 1.45     |
| 930196       | FRONTIER        | 18               | 378,000    | 64.198 µs            | 124.53 µs          | 1.94     |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 90.579 µs            | 117.67 µs          | 1.3      |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 824.90 µs            | 1.4999 ms          | 1.82     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 1.6504 ms            | 1.8854 ms          | 1.14     |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 353.64 µs            | 627.91 µs          | 1.78     |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 134.05 µs            | 119.58 µs          | **0.89** |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 120.42 µs            | 125.91 µs          | 1.05     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.3193 ms            | 644.19 µs          | **0.49** |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 785.45 µs            | 385.63 µs          | **0.49** |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.6464 ms            | 2.4256 ms          | **0.92** |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 1.3198 ms            | 2.4326 ms          | 1.84     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.1367 ms            | 1.3499 ms          | **0.33** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.8366 ms            | 2.4461 ms          | **0.51** |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.5213 ms            | 1.4171 ms          | **0.4**  |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 783.39 µs            | 961.55 µs          | 1.23     |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 4.4349 ms            | 2.8628 ms          | **0.65** |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 2.2744 ms            | 3.7299 ms          | 1.64     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 6.2085 ms            | 3.1359 ms          | **0.51** |
| 12244000     | BERLIN          | 133              | 12,450,737 | 10.676 ms            | 8.1790 ms          | **0.77** |
| 12300570     | BERLIN          | 687              | 14,934,316 | 3.0600 ms            | 4.3783 ms          | 1.43     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 4.2419 ms            | 5.6387 ms          | 1.33     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 4.0665 ms            | 2.0490 ms          | **0.5**  |
| 12964999     | BERLIN          | 145              | 15,026,712 | 12.218 ms            | 10.069 ms          | **0.82** |
| 12965000     | LONDON          | 259              | 30,025,257 | 23.524 ms            | 8.1404 ms          | **0.35** |
| 13217637     | LONDON          | 1100             | 29,985,362 | 9.6725 ms            | 7.8234 ms          | **0.81** |
| 13287210     | LONDON          | 1414             | 29,990,789 | 5.5385 ms            | 10.135 ms          | 1.83     |
| 14029313     | LONDON          | 724              | 30,074,554 | 10.050 ms            | 2.9390 ms          | **0.29** |
| 14334629     | LONDON          | 819              | 30,135,754 | 13.423 ms            | 6.6662 ms          | **0.5**  |
| 14383540     | LONDON          | 722              | 30,059,751 | 14.340 ms            | 6.1922 ms          | **0.43** |
| 14396881     | LONDON          | 1346             | 30,020,813 | 6.5816 ms            | 9.3410 ms          | 1.42     |
| 15199017     | LONDON          | 866              | 30,028,395 | 10.753 ms            | 5.0941 ms          | **0.47** |
| 15537393     | LONDON          | 1                | 29,991,429 | 11.539 µs            | 13.292 µs          | 1.15     |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.9696 ms            | 1.9209 ms          | **0.65** |
| 15538827     | MERGE           | 823              | 29,981,465 | 13.216 ms            | 7.6968 ms          | **0.58** |
| 16146267     | MERGE           | 473              | 19,204,593 | 9.1232 ms            | 3.3758 ms          | **0.37** |
| 17034869     | MERGE           | 93               | 8,450,250  | 5.1528 ms            | 2.5558 ms          | **0.5**  |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 13.692 ms            | 9.0195 ms          | **0.66** |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 16.447 ms            | 8.2956 ms          | **0.5**  |
| 19426586     | SHANGHAI        | 127              | 1,5757,891 | 10.662 ms            | 8.8176 ms          | **0.83** |
| 19426587     | CANCUN          | 37               | 2,633,933  | 2.2226 ms            | 1.3733 ms          | **0.62** |
| 19638737     | CANCUN          | 381              | 15,932,416 | 9.8232 ms            | 6.6009 ms          | **0.67** |
| 19807137     | CANCUN          | 712              | 29,981,386 | 21.256 ms            | 10.197 ms          | **0.48** |
| 19917570     | CANCUN          | 116              | 12,889,065 | 8.5545 ms            | 4.3840 ms          | **0.51** |
| 19923400     | CANCUN          | 24               | 1,624,049  | 1.2734 ms            | 1.0750 ms          | **0.84** |
| 19929064     | CANCUN          | 103              | 7,743,849  | 5.0080 ms            | 2.9115 ms          | **0.58** |
| 19932148     | CANCUN          | 227              | 14,378,808 | 10.159 ms            | 6.0995 ms          | **0.6**  |
| 19932703     | CANCUN          | 143              | 10,421,765 | 11.891 ms            | 6.9915 ms          | **0.59** |
| 19932810     | CANCUN          | 270              | 18,643,597 | 12.847 ms            | 7.7446 ms          | **0.6**  |
| 19933122     | CANCUN          | 45               | 2,056,821  | 913.59 µs            | 621.96 µs          | **0.68** |
| 19933597     | CANCUN          | 154              | 12,788,678 | 6.1444 ms            | 3.9447 ms          | **0.64** |
| 19933612     | CANCUN          | 130              | 11,236,414 | 10.222 ms            | 4.0232 ms          | **0.39** |
| 19934116     | CANCUN          | 58               | 3,365,857  | 2.3129 ms            | 1.4174 ms          | **0.61** |

## Gigagas

This benchmark includes mocked blocks that exceed 1 Gigagas to see how PEVM can speed up building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup.

|                 | No. Transactions | Gas Used      | Sequential Execution | Parallel Execution | P / S    |
| --------------- | ---------------- | ------------- | -------------------- | ------------------ | -------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 149.74 ms            | 111.98 ms          | **0.75** |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 225.16 ms            | 85.193 ms          | **0.38** |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 563.92 ms            | 65.363 ms          | **0.12** |
