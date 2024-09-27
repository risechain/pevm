//! PEVM Strategies: Sequential and Parallel

use std::sync::LazyLock;

static AVAILABLE_PARALLELISM: LazyLock<usize> =
    LazyLock::new(|| match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    });

/// Configuration for parallel execution.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of threads to use.
    pub num_threads: usize,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        // This max should be tuned to the running machine,
        // ideally also per block depending on the number of
        // transactions, gas usage, etc. ARM machines seem to
        // go higher thanks to their low thread overheads.
        let num_threads = AVAILABLE_PARALLELISM.min(
            #[cfg(target_arch = "aarch64")]
            12,
            #[cfg(not(target_arch = "aarch64"))]
            8,
        );

        Self { num_threads }
    }
}

/// Execution strategy for Pevm.
#[derive(Debug, Clone)]
pub enum PevmStrategy {
    /// Sequential execution.
    Sequential,

    /// Parallel execution with configuration.
    Parallel {
        /// Parallel execution configuration.
        config: ParallelConfig,
    },
}

impl From<ParallelConfig> for PevmStrategy {
    fn from(config: ParallelConfig) -> Self {
        Self::Parallel { config }
    }
}

impl PevmStrategy {
    /// Requires PEVM to run sequentially.
    pub fn sequential() -> Self {
        Self::Sequential
    }

    /// Decides whether to run sequentially or in parallel.
    pub fn auto(num_txs: usize, block_gas_used: u128) -> Self {
        // TODO: Continue to fine tune this condition.
        if block_gas_used < 4_000_000 {
            return Self::Sequential;
        }
        let parallel_config = ParallelConfig::default();
        if num_txs < parallel_config.num_threads {
            return Self::Sequential;
        }
        Self::Parallel {
            config: parallel_config,
        }
    }
}
