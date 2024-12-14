# Benchmarks

We use [criterion.rs](https://github.com/bheisler/criterion.rs) to benchmark 100 samples for each sequential and parallel execution of a block.

For simplicity, we load the chain state into memory before execution. **In practice**, programs load chain states from disk with some in-memory cache, which **increases speedup for parallel execution** as disk I/O only blocks the reading thread, while other threads still execute and validate with in-memory data. On the other hand, sequential execution is completely blocked every time it reads new data from the disk.

> :warning: **Warning**
> Micro-benchmarking multithreaded programs is rather nuanced. For maximal accuracy, ensure no heavy processes are running in the background and the benchmark machine is run in high-performance (as opposed to power-saving) mode. If the benchmark machine has more performance cores than the concurrency level (typically 8-12 for Ethereum mainnet blocks), it is best to benchmark with only performance cores like with:
> `$ taskset -c -a 0-15 cargo bench --features global-alloc --bench mainnet`
> Mixing efficient cores can degrade speedup by over 25%.

The tables below were produced on a `c7g.8xlarge` EC2 instance with Graviton3 (32 vCPUs @2.6 GHz).

## Gigagas Blocks

This benchmark includes mocked 1-Gigagas blocks to see how pevm aids in building and syncing large blocks going forward. All blocks are in the CANCUN spec with no dependencies to measure the maximum speedup. We pick `jemalloc` with THP as the global memory allocator, which performs the best for big blocks. `rpmalloc` is much better for the Uniswap case, but much worse on the others and is not stable on AWS Graviton.

The benchmark runs with a single transaction type, not representing real-world blocks on a universal L2. However, it may be representative of application-specific L2s.

To run the benchmark yourself:

```sh
$ JEMALLOC_SYS_WITH_MALLOC_CONF="thp:always,metadata_thp:always" cargo bench --features global-alloc --bench gigagas
```

|                 | No. Transactions | Gas Used      | Sequential (ms) | Parallel (ms) | Speedup    |
| --------------- | ---------------- | ------------- | --------------- | ------------- | ---------- |
| Raw Transfers   | 47,620           | 1,000,020,000 | 159.08          | 56.425        | 🟢2.82     |
| ERC20 Transfers | 37,123           | 1,000,019,374 | 246.43          | 60.817        | 🟢4.05     |
| Uniswap Swaps   | 6,413            | 1,000,004,742 | 413.42          | 18.707        | 🟢**22.1** |

## Ethereum Mainnet Blocks

This benchmark includes several transactions for each Ethereum hardfork that alters the EVM spec. We include blocks with high parallelism, highly inter-dependent blocks, and some random blocks to ensure we benchmark against all scenarios. It is also a good testing platform for aggressively running blocks to find race conditions if there are any.

The current hardcoded concurrency level is 8 on x86 and 12 on ARM, which have performed best for Ethereum blocks thus far. Increasing it will improve results for blocks with more parallelism but hurt small or highly interdependent blocks due to thread overheads. Ideally, our static analysis will be smart enough to auto-tune this better.

We pick `rpmalloc` for x86 and `snmalloc` for ARM as the global memory allocator. `rpmalloc` is generally better but can crash on AWS Graviton.

To run the benchmark yourself:

```sh
$ cargo bench --features global-alloc --bench mainnet
```

To benchmark with profiling for development (preferably after commenting out the sequential run):

```sh
# Higher level with flamegraph
$ cargo flamegraph --profile profiling --bench mainnet -- --bench

# Lower level with perf
$ cargo bench --profile profiling --bench mainnet
$ perf record target/profiling/deps/mainnet-??? --bench
$ perf report
```

| Block Number | Spec            | No. Transactions | Gas Used   | Sequential (ms) | Parallel (ms) | Speedup    |
| ------------ | --------------- | ---------------- | ---------- | --------------- | ------------- | ---------- |
| 46147        | FRONTIER        | 1                | 21,000     | 0.004           | 0.004         | ⚪1        |
| 116525       | FRONTIER        | 83               | 2,625,335  | 0.267           | 0.268         | ⚪1        |
| 930196       | FRONTIER        | 18               | 378,000    | 0.048           | 0.048         | ⚪1        |
| 1150000      | HOMESTEAD       | 9                | 649,041    | 0.098           | 0.098         | ⚪1        |
| 1796867      | HOMESTEAD       | 49               | 3,917,663  | 0.334           | 0.334         | ⚪1        |
| 2179522      | HOMESTEAD       | 222              | 4,698,004  | 0.604           | 0.506         | 🟢1.19     |
| 2462997      | HOMESTEAD       | 9                | 484,186    | 3.429           | 3.405         | ⚪1        |
| 2641321      | TANGERINE       | 83               | 1,917,429  | 0.278           | 0.279         | ⚪1        |
| 2674998      | TANGERINE       | 16               | 1,915,348  | 0.124           | 0.125         | ⚪1        |
| 2675000      | SPURIOUS DRAGON | 15               | 1,312,529  | 0.108           | 0.108         | ⚪1        |
| 2688148      | SPURIOUS DRAGON | 4                | 2,725,844  | 0.18            | 0.18          | ⚪1        |
| 3356896      | SPURIOUS DRAGON | 176              | 4,033,966  | 0.569           | 0.451         | 🟢1.26     |
| 4330482      | SPURIOUS DRAGON | 237              | 6,669,817  | 1.04            | 0.518         | 🟢2.01     |
| 4369999      | SPURIOUS DRAGON | 22               | 6,630,311  | 0.98            | 0.602         | 🟢1.63     |
| 4370000      | BYZANTIUM       | 97               | 6,609,719  | 2.044           | 1.992         | 🟢1.03     |
| 4864590      | BYZANTIUM       | 195              | 7,985,890  | 2.584           | 0.728         | 🟢3.55     |
| 5283152      | BYZANTIUM       | 150              | 7,988,261  | 2.552           | 0.682         | 🟢3.74     |
| 5526571      | BYZANTIUM       | 143              | 7,988,261  | 2.066           | 0.922         | 🟢2.24     |
| 5891667      | BYZANTIUM       | 380              | 7,980,153  | 0.922           | 0.653         | 🟢1.41     |
| 6137495      | BYZANTIUM       | 60               | 7,994,690  | 1.246           | 0.691         | 🟢1.8      |
| 6196166      | BYZANTIUM       | 108              | 7,975,867  | 1.052           | 0.969         | 🟢1.08     |
| 7279999      | BYZANTIUM       | 122              | 7,998,886  | 4.54            | 1.05          | 🟢**4.32** |
| 7280000      | PETERSBURG      | 118              | 7,992,790  | 4.505           | 2.327         | 🟢1.94     |
| 8038679      | PETERSBURG      | 237              | 7,993,635  | 2.168           | 0.922         | 🟢2.35     |
| 8889776      | PETERSBURG      | 330              | 9,996,021  | 3.062           | 1.216         | 🟢2.52     |
| 9068998      | PETERSBURG      | 3                | 3,575,534  | 0.804           | 0.803         | ⚪1        |
| 9069000      | ISTANBUL        | 56               | 8,762,935  | 3.994           | 2.284         | 🟢1.75     |
| 10760440     | ISTANBUL        | 202              | 12,466,618 | 5.071           | 2.02          | 🟢2.51     |
| 11114732     | ISTANBUL        | 100              | 12,450,745 | 3.605           | 4.06          | 🔴0.89     |
| 11743952     | ISTANBUL        | 206              | 11,955,916 | 11.812          | 12.64         | 🔴0.93     |
| 11814555     | ISTANBUL        | 579              | 12,494,001 | 1.658           | 1.085         | 🟢1.53     |
| 12047794     | ISTANBUL        | 232              | 12,486,404 | 4.313           | 4.779         | 🔴0.9      |
| 12159808     | ISTANBUL        | 180              | 12,478,883 | 4.183           | 4.722         | 🔴0.89     |
| 12243999     | ISTANBUL        | 205              | 12,444,977 | 4.641           | 1.677         | 🟢2.77     |
| 12244000     | BERLIN          | 133              | 12,450,737 | 6.927           | 4.362         | 🟢1.59     |
| 12300570     | BERLIN          | 687              | 14,934,316 | 2.103           | 1.252         | 🟢1.68     |
| 12459406     | BERLIN          | 201              | 14,994,849 | 7.126           | 4.285         | 🟢1.66     |
| 12520364     | BERLIN          | 660              | 14,989,902 | 2.758           | 1.658         | 🟢1.66     |
| 12522062     | BERLIN          | 177              | 15,028,295 | 3.102           | 1.292         | 🟢2.4      |
| 12964999     | BERLIN          | 145              | 15,026,712 | 9.856           | 5.229         | 🟢1.89     |
| 12965000     | LONDON          | 259              | 30,025,257 | 22.032          | 6.27          | 🟢3.51     |
| 13217637     | LONDON          | 1100             | 29,985,362 | 8.221           | 2.736         | 🟢3.01     |
| 13287210     | LONDON          | 1414             | 29,990,789 | 4.232           | 2.546         | 🟢1.66     |
| 14029313     | LONDON          | 724              | 30,074,554 | 7.488           | 2.003         | 🟢3.74     |
| 14334629     | LONDON          | 819              | 30,135,754 | 9.826           | 3.181         | 🟢3.09     |
| 14383540     | LONDON          | 722              | 30,059,751 | 11.582          | 3.853         | 🟢3.01     |
| 14396881     | LONDON          | 1346             | 30,020,813 | 4.856           | 2.708         | 🟢1.79     |
| 14545870     | LONDON          | 456              | 29,925,884 | 13.234          | 3.861         | 🟢3.43     |
| 15199017     | LONDON          | 866              | 30,028,395 | 8.7             | 2.551         | 🟢3.41     |
| 15274915     | LONDON          | 1226             | 29,928,443 | 6.048           | 2.626         | 🟢2.3      |
| 15537393     | LONDON          | 1                | 29,991,429 | 2.25            | 2.258         | ⚪1        |
| 15537394     | MERGE           | 80               | 29,983,006 | 2.684           | 1.721         | 🟢1.56     |
| 15538827     | MERGE           | 823              | 29,981,465 | 9.288           | 3.026         | 🟢3.07     |
| 15752489     | MERGE           | 132              | 8,242,594  | 2.902           | 1.331         | 🟢2.18     |
| 16146267     | MERGE           | 473              | 19,204,593 | 8.114           | 2.842         | 🟢2.86     |
| 16257471     | MERGE           | 98               | 20,267,875 | 11.864          | 7.487         | 🟢1.58     |
| 17034869     | MERGE           | 93               | 8,450,250  | 3.685           | 1.555         | 🟢2.37     |
| 17034870     | SHANGHAI        | 184              | 29,999,074 | 10.328          | 4.429         | 🟢2.33     |
| 17666333     | SHANGHAI        | 961              | 29,983,414 | 14.424          | 7.185         | 🟢2.01     |
| 18085863     | SHANGHAI        | 178              | 17,007,666 | 7.847           | 4.315         | 🟢1.84     |
| 18426253     | SHANGHAI        | 147              | 18,889,343 | 12.143          | 8.15          | 🟢1.49     |
| 18988207     | SHANGHAI        | 186              | 12,398,324 | 12.442          | 7.812         | 🟢1.59     |
| 19426586     | SHANGHAI        | 127              | 15,757,891 | 8.032           | 3.79          | 🟢2.12     |
| 19426587     | CANCUN          | 37               | 2,633,933  | 3.215           | 3.219         | ⚪1        |
| 19444337     | CANCUN          | 417              | 29,999,800 | 15.2            | 4.698         | 🟢3.24     |
| 19469101     | CANCUN          | 469              | 26,398,517 | 16.218          | 7.531         | 🟢2.15     |
| 19498855     | CANCUN          | 241              | 29,919,049 | 17.125          | 8.174         | 🟢2.1      |
| 19505152     | CANCUN          | 417              | 29,999,872 | 14.67           | 4.536         | 🟢3.23     |
| 19606599     | CANCUN          | 367              | 29,981,684 | 22.352          | 8.72          | 🟢2.56     |
| 19638737     | CANCUN          | 381              | 15,932,416 | 6.472           | 3.214         | 🟢2.01     |
| 19716145     | CANCUN          | 341              | 29,995,804 | 13.552          | 5.887         | 🟢2.3      |
| 19737292     | CANCUN          | 195              | 29,999,921 | 9.427           | 3.799         | 🟢2.48     |
| 19807137     | CANCUN          | 712              | 29,981,386 | 20.228          | 9.77          | 🟢2.07     |
| 19860366     | CANCUN          | 430              | 29,969,358 | 13.965          | 5.393         | 🟢2.59     |
| 19910734     | CANCUN          | 0                | 0          | 0.002           | 0.002         | ⚪1        |
| 19917570     | CANCUN          | 116              | 12,889,065 | 5.762           | 2.161         | 🟢2.67     |
| 19923400     | CANCUN          | 24               | 1,624,049  | 0.724           | 0.726         | ⚪1        |
| 19929064     | CANCUN          | 103              | 7,743,849  | 3.749           | 1.879         | 🟢2        |
| 19932148     | CANCUN          | 227              | 14,378,808 | 6.939           | 3.332         | 🟢2.08     |
| 19932703     | CANCUN          | 143              | 10,421,765 | 13.058          | 9.369         | 🟢1.39     |
| 19932810     | CANCUN          | 270              | 18,643,597 | 7.935           | 3.62          | 🟢2.19     |
| 19933122     | CANCUN          | 45               | 2,056,821  | 0.67            | 0.672         | ⚪1        |
| 19933597     | CANCUN          | 154              | 12,788,678 | 4.177           | 2.351         | 🟢1.78     |
| 19933612     | CANCUN          | 130              | 11,236,414 | 7.482           | 2.059         | 🟢3.63     |
| 19934116     | CANCUN          | 58               | 3,365,857  | 1.77            | 1.761         | ⚪1        |

- We are currently **~2.02 times faster than sequential execution** on average.
- The **max speed-up is x4.32** for a block with few dependencies.
- The **max slow-down is x0.89** for a block that self-destructs then redeploys the same contract within, which forces us to fall back to sequential at the moment.
- We will need more optimizations throughout Alpha and Beta to become **3~5 times faster**.
