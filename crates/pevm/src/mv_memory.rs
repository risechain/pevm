use std::{
    cell::UnsafeCell,
    collections::{BTreeMap, HashSet},
    sync::Mutex,
};

use alloy_primitives::{Address, B256};
use dashmap::DashMap;
use revm::state::Bytecode;

use crate::{
    BuildIdentityHasher, BuildSuffixHasher, MemoryEntry, MemoryLocation, MemoryLocationHash,
    ReadOrigin, ReadSet, TxIdx, TxVersion, WriteSet, hash_deterministic,
};

#[derive(Default, Debug)]
struct LastLocations {
    read: ReadSet,
    // Consider [SmallVec] since most transactions explicitly write to 2 locations!
    write: Vec<MemoryLocationHash>,
}

type LazyAddresses = HashSet<Address, BuildSuffixHasher>;

// Per-location multi-version storage, in one of two layouts:
//
// Sparse — a BTreeMap keyed by tx_idx, used for most memory locations (storage slots,
// code hashes, etc.) where only a handful of transactions write to the same location.
// Supports O(log n) range queries required by validate_read_locations.
//
// Dense — a contiguous boxed slice indexed directly by tx_idx, used for lazy addresses
// (beneficiary, fee recipients, raw-transfer accounts) which can receive writes from
// nearly every transaction in the block. Dense gives O(1) indexed writes and
// cache-friendly sequential iteration during post-processing, replacing the
// pointer-chasing BTreeMap for these hot paths.
//
// SAFETY invariant for Dense: the Block-STM scheduler ensures at most one thread writes
// to a given tx_idx slot at a time, so UnsafeCell slots on different indices never race.
pub(crate) enum MvEntries {
    Sparse(BTreeMap<TxIdx, MemoryEntry>),
    Dense(Box<[UnsafeCell<Option<MemoryEntry>>]>),
}

// SAFETY: UnsafeCell inside Dense is protected by the scheduler's per-tx-idx exclusion.
unsafe impl Send for MvEntries {}
unsafe impl Sync for MvEntries {}

impl std::fmt::Debug for MvEntries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sparse(m) => write!(f, "Sparse(len={})", m.len()),
            Self::Dense(s) => write!(f, "Dense(len={})", s.len()),
        }
    }
}

impl MvEntries {
    fn new_dense(block_size: usize) -> Self {
        Self::Dense(
            (0..block_size)
                .map(|_| UnsafeCell::new(None))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }

    // Returns the inner BTreeMap for Sparse entries.
    // Dense entries are for lazy addresses whose reads are mocked in the VM (is_lazy=true),
    // so they never reach the MvMemory range-query path. Returning None is correct there.
    pub(crate) fn as_sparse(&self) -> Option<&BTreeMap<TxIdx, MemoryEntry>> {
        match self {
            Self::Sparse(map) => Some(map),
            Self::Dense(_) => None,
        }
    }

    // Iterate Dense slots in tx_idx order, returning references to entries or None.
    // SAFETY: must be called only in single-threaded post-processing after all writes finish.
    pub(crate) fn dense_iter(&self) -> impl Iterator<Item = Option<&MemoryEntry>> {
        let Self::Dense(slots) = self else { unreachable!() };
        slots.iter().map(|cell| unsafe { (*cell.get()).as_ref() })
    }
}

/// The `MvMemory` contains shared memory in a form of a multi-version data
/// structure for values written and read by different transactions. It stores
/// multiple writes for each memory location, along with a value and an associated
/// version of a corresponding transaction.
#[derive(Debug)]
pub struct MvMemory {
    /// Per-location multi-version entries.
    /// Sparse (BTreeMap) for most locations; Dense (Vec indexed by tx_idx) for lazy addresses.
    // No more hashing is required as we already identify memory locations by their hash
    // in the read & write sets.
    pub(crate) data: DashMap<MemoryLocationHash, MvEntries, BuildIdentityHasher>,
    /// Total number of transactions; needed to size new Dense entries added at runtime.
    block_size: usize,
    /// Last read & written locations of each transaction.
    last_locations: Vec<Mutex<LastLocations>>,
    /// Lazy addresses that need full evaluation at the end of the block.
    lazy_addresses: Mutex<LazyAddresses>,
    /// New bytecodes deployed in this block.
    pub(crate) new_bytecodes: DashMap<B256, Bytecode, BuildSuffixHasher>,
}

impl MvMemory {
    pub(crate) fn new(
        block_size: usize,
        estimated_locations: impl IntoIterator<Item = (MemoryLocationHash, Vec<TxIdx>)>,
        lazy_addresses: impl IntoIterator<Item = Address>,
    ) -> Self {
        let data: DashMap<MemoryLocationHash, MvEntries, BuildIdentityHasher> =
            DashMap::default();

        // Pre-populate estimated locations with Sparse Estimate entries to avoid
        // BTreeMap rebalancing under lock at runtime.
        for (location_hash, estimated_tx_idxs) in estimated_locations {
            data.insert(
                location_hash,
                MvEntries::Sparse(
                    estimated_tx_idxs
                        .into_iter()
                        .map(|tx_idx| (tx_idx, MemoryEntry::Estimate))
                        .collect(),
                ),
            );
        }

        // Lazy addresses use Dense entries so writes are O(1) indexed and post-processing
        // iterates a contiguous slice instead of a pointer-chasing BTreeMap.
        let lazy_addresses_vec: Vec<Address> = lazy_addresses.into_iter().collect();
        for &address in &lazy_addresses_vec {
            let hash = hash_deterministic(MemoryLocation::Basic(address));
            // Overwrite any Sparse entry (e.g. from estimated_locations) with Dense.
            data.insert(hash, MvEntries::new_dense(block_size));
        }

        Self {
            data,
            block_size,
            last_locations: (0..block_size).map(|_| Mutex::default()).collect(),
            lazy_addresses: Mutex::new(LazyAddresses::from_iter(lazy_addresses_vec)),
            new_bytecodes: DashMap::default(),
        }
    }

    pub(crate) fn add_lazy_addresses(&self, new_lazy_addresses: impl IntoIterator<Item = Address>) {
        let mut lazy_addresses = self.lazy_addresses.lock().unwrap();
        for address in new_lazy_addresses {
            if !lazy_addresses.insert(address) {
                continue; // already registered; Dense entry already exists in data
            }
            let hash = hash_deterministic(MemoryLocation::Basic(address));
            // Upgrade the location from Sparse to Dense atomically under the DashMap shard
            // write lock. Holding the lock through the entire replace+migrate prevents a
            // concurrent record() from writing to a Sparse we're about to drop.
            let mut entry_ref = self
                .data
                .entry(hash)
                .or_insert_with(|| MvEntries::Sparse(BTreeMap::new()));
            let old = std::mem::replace(entry_ref.value_mut(), MvEntries::new_dense(self.block_size));
            // Migrate existing Sparse Data entries into Dense so post-processing only looks there.
            if let MvEntries::Sparse(map) = old {
                let MvEntries::Dense(slots) = entry_ref.value() else { unreachable!() };
                for (tx_idx, entry) in map {
                    if let MemoryEntry::Data(..) = &entry {
                        // SAFETY: no concurrent writes during add_lazy_addresses migration
                        // since we hold the DashMap shard write lock.
                        unsafe { *slots[tx_idx].get() = Some(entry) };
                    }
                    // Skip Estimate entries; they'll be overwritten on re-execution.
                }
            }
        }
    }

    // Apply a new pair of read & write sets to the multi-version data structure.
    // Return whether a write occurred to a memory location not written to by
    // the previous incarnation of the same transaction. This determines whether
    // the executed higher transactions need re-validation.
    pub(crate) fn record(
        &self,
        tx_version: &TxVersion,
        read_set: ReadSet,
        write_set: WriteSet,
    ) -> bool {
        let mut last_locations = index_mutex!(self.last_locations, tx_version.tx_idx);
        last_locations.read = read_set;

        // Remove stale writes from the previous incarnation.
        let mut last_location_idx = 0;
        while last_location_idx < last_locations.write.len() {
            let prev_location = unsafe { last_locations.write.get_unchecked(last_location_idx) };
            if write_set.iter().all(|(l, _)| l != prev_location) {
                if let Some(mut e) = self.data.get_mut(prev_location) {
                    match e.value_mut() {
                        MvEntries::Dense(slots) => {
                            // SAFETY: see struct-level invariant.
                            unsafe { *slots[tx_version.tx_idx].get() = None };
                        }
                        MvEntries::Sparse(map) => {
                            map.remove(&tx_version.tx_idx);
                        }
                    }
                }
                last_locations.write.swap_remove(last_location_idx);
            } else {
                last_location_idx += 1;
            }
        }

        // Register new writes.
        let mut wrote_new_location = false;

        for (location, value) in write_set {
            // Fast path: Dense entries allow lockless indexed writes via a read lock.
            // Clone value for the Dense case; move it for Sparse (avoids Clone on cold path).
            let used_dense = if let Some(e) = self.data.get(&location) {
                if let MvEntries::Dense(slots) = e.value() {
                    // SAFETY: see struct-level invariant.
                    unsafe {
                        *slots[tx_version.tx_idx].get() =
                            Some(MemoryEntry::Data(tx_version.tx_incarnation, value.clone()));
                    };
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if !used_dense {
                // Sparse path: acquire write lock. Handle race where add_lazy_addresses
                // upgraded this location to Dense between our get() check and entry().
                let new_entry = MemoryEntry::Data(tx_version.tx_incarnation, value);
                let mut entry_ref = self
                    .data
                    .entry(location)
                    .or_insert_with(|| MvEntries::Sparse(BTreeMap::new()));
                match entry_ref.value_mut() {
                    MvEntries::Sparse(map) => {
                        map.insert(tx_version.tx_idx, new_entry);
                    }
                    MvEntries::Dense(slots) => {
                        // SAFETY: see struct-level invariant.
                        unsafe { *slots[tx_version.tx_idx].get() = Some(new_entry) };
                    }
                }
            }

            if !last_locations.write.contains(&location) {
                last_locations.write.push(location);
                wrote_new_location = true;
            }
        }

        wrote_new_location
    }

    // Obtain the last read set recorded by an execution of [tx_idx] and check
    // that re-reading each memory location in the read set still yields the
    // same read origins.
    // This is invoked during validation, when the incarnation being validated is
    // already executed and has recorded the read set. However, if the thread
    // performing a validation for incarnation i of a transaction is slow, it is
    // possible that this function invocation observes a read set recorded by a
    // latter (> i) incarnation. In this case, incarnation i is guaranteed to be
    // already aborted (else higher incarnations would never start), and the
    // validation task will have no effect regardless of the outcome (only
    // validations that successfully abort affect the state and each incarnation
    // can be aborted at most once).
    pub(crate) fn validate_read_locations(&self, tx_idx: TxIdx) -> bool {
        for (location, prior_origins) in &index_mutex!(self.last_locations, tx_idx).read {
            if let Some(entries) = self.data.get(location) {
                match entries.value() {
                    MvEntries::Sparse(map) => {
                        let mut iter = map.range(..tx_idx);
                        for prior_origin in prior_origins {
                            if let ReadOrigin::MvMemory(prior_version) = prior_origin {
                                // Found something: Must match version.
                                if let Some((closest_idx, MemoryEntry::Data(tx_incarnation, ..))) =
                                    iter.next_back()
                                {
                                    if closest_idx != &prior_version.tx_idx
                                        || &prior_version.tx_incarnation != tx_incarnation
                                    {
                                        return false;
                                    }
                                }
                                // The previously read value is now cleared or marked ESTIMATE.
                                else {
                                    return false;
                                }
                            }
                            // Read from storage but there is now something in between!
                            else if iter.next_back().is_some() {
                                return false;
                            }
                        }
                    }
                    MvEntries::Dense(slots) => {
                        // Scan backward from tx_idx, maintaining a cursor so successive
                        // next_back() calls don't re-scan already-checked indices.
                        // Dense validation is rare (lazy-address reads are mocked in the VM),
                        // so the O(tx_idx) scan is acceptable.
                        let mut cursor = tx_idx;
                        for prior_origin in prior_origins {
                            let found = (0..cursor).rev().find_map(|i| {
                                // SAFETY: validation runs after the corresponding record() with
                                // a happens-before edge enforced by the scheduler's atomic state.
                                let entry = unsafe { (*slots[i].get()).as_ref() }?;
                                Some((i, entry))
                            });
                            if let ReadOrigin::MvMemory(prior_version) = prior_origin {
                                if let Some((closest_idx, MemoryEntry::Data(tx_incarnation, ..))) =
                                    found
                                {
                                    if closest_idx != prior_version.tx_idx
                                        || &prior_version.tx_incarnation != tx_incarnation
                                    {
                                        return false;
                                    }
                                    cursor = closest_idx;
                                } else {
                                    return false;
                                }
                            } else if found.is_some() {
                                return false;
                            }
                        }
                    }
                }
            }
            // Read from multi-version data but now it's cleared.
            else if prior_origins.len() != 1 || prior_origins.last() != Some(&ReadOrigin::Storage)
            {
                return false;
            }
        }
        true
    }

    // Replace the write set of the aborted version in the shared memory data
    // structure with special ESTIMATE markers to quickly abort higher transactions
    // that read them.
    pub(crate) fn convert_writes_to_estimates(&self, tx_idx: TxIdx) {
        for location in &index_mutex!(self.last_locations, tx_idx).write {
            if let Some(mut e) = self.data.get_mut(location) {
                match e.value_mut() {
                    MvEntries::Dense(slots) => {
                        // Clear the slot so re-execution writes fresh data. Dense locations
                        // don't use Estimate markers because lazy-address reads are mocked
                        // in the VM and never block waiting for an Estimate to resolve.
                        unsafe { *slots[tx_idx].get() = None };
                    }
                    MvEntries::Sparse(map) => {
                        map.insert(tx_idx, MemoryEntry::Estimate);
                    }
                }
            }
        }
    }

    pub(crate) fn consume_lazy_addresses(&self) -> impl IntoIterator<Item = Address> {
        std::mem::take(&mut *self.lazy_addresses.lock().unwrap()).into_iter()
    }
}
