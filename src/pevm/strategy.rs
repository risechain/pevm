//! PEVM Strategies: Sequential and Parallel

use std::{collections::BinaryHeap, sync::LazyLock};

use revm::primitives::TxEnv;

use crate::TxIdx;

static AVAILABLE_PARALLELISM: LazyLock<usize> =
    LazyLock::new(|| match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    });

/// Configuration for parallel execution.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of threads for regular transactions
    pub num_threads_for_regular_txs: usize,
    /// Number of threads for priority transactions
    pub num_threads_for_priority_txs: usize,
    /// Max number of priority transactions
    pub max_num_priority_txs: usize,
}

impl ParallelConfig {
    /// Create [ParallelConfig]
    pub fn new(
        num_threads_for_regular_txs: usize,
        num_threads_for_priority_txs: usize,
        max_num_priority_txs: usize,
    ) -> Self {
        Self {
            num_threads_for_regular_txs,
            num_threads_for_priority_txs,
            max_num_priority_txs,
        }
    }
}

impl Default for ParallelConfig {
    fn default() -> Self {
        // TODO: Fine tune these parameters based on arch.
        let num_threads_for_regular_txs = AVAILABLE_PARALLELISM.min(12);
        let num_threads_for_priority_txs = AVAILABLE_PARALLELISM
            .saturating_sub(num_threads_for_regular_txs)
            .min(4);
        let max_num_priority_txs = 24;
        Self {
            num_threads_for_regular_txs,
            num_threads_for_priority_txs,
            max_num_priority_txs,
        }
    }
}

impl ParallelConfig {
    /// Returns the list of priority transactions.
    pub fn get_priority_txs(&self, txs: &[TxEnv]) -> Vec<TxIdx> {
        if self.max_num_priority_txs == 0 || self.num_threads_for_priority_txs == 0 {
            return Vec::new();
        }
        // [std::collections::BinaryHeap] is a max heap.
        // While pushing the txs to the heap, every time the size exceeds
        // [self.max_num_priority_txs], we pop the lightest tx.
        // At the end, the heap contains [self.max_num_priority_txs] heaviest txs.
        let mut heap = BinaryHeap::with_capacity(self.max_num_priority_txs + 1);
        for (tx_idx, tx_env) in txs.iter().enumerate() {
            heap.push((!tx_env.gas_limit, tx_idx));
            if heap.len() > self.max_num_priority_txs {
                heap.pop();
            }
        }

        let mut priority_txs = Vec::with_capacity(heap.len());
        while let Some((_, tx_idx)) = heap.pop() {
            priority_txs.push(tx_idx);
        }
        priority_txs.reverse();
        priority_txs
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
        if num_txs
            < parallel_config.num_threads_for_priority_txs
                + parallel_config.num_threads_for_regular_txs
        {
            return Self::Sequential;
        }

        if num_txs <= 384 && *AVAILABLE_PARALLELISM >= 12 {
            // Reduce the number of regular workers to optimize for small blocks
            return Self::Parallel {
                config: ParallelConfig {
                    num_threads_for_regular_txs: 8,
                    num_threads_for_priority_txs: 4,
                    max_num_priority_txs: 24,
                },
            };
        }

        Self::Parallel {
            config: parallel_config,
        }
    }
}
