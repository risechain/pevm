use std::{collections::BTreeMap, sync::Mutex};

use dashmap::{
    mapref::{entry::Entry, one::Ref},
    DashMap,
};

use crate::{
    BuildIdentityHasher, MemoryEntry, MemoryLocationHash, ReadLocations, ReadOrigin, TxIdx,
    TxVersion, WriteSet,
};

#[derive(Default)]
struct LastLocations {
    read: ReadLocations,
    // Consider [SmallVec] since most transactions explicitly write to 2 locations!
    write: Vec<MemoryLocationHash>,
}

// The MvMemory contains shared memory in a form of a multi-version data
// structure for values written and read by different transactions. It stores
// multiple writes for each memory location, along with a value and an associated
// version of a corresponding transaction.
pub(crate) struct MvMemory {
    // No more hashing is required as we already identify memory locations by their hash
    // in the read & write sets. [dashmap] having a dedicated interface for this use case
    // (that skips hashing for [u64] keys) would make our code cleaner and "faster".
    // Nevertheless, the compiler should be good enough to optimize these cases anyway.
    data: DashMap<MemoryLocationHash, BTreeMap<TxIdx, MemoryEntry>, BuildIdentityHasher>,
    last_locations: Vec<Mutex<LastLocations>>,
}

impl MvMemory {
    pub(crate) fn new(
        block_size: usize,
        estimated_locations: impl IntoIterator<Item = (MemoryLocationHash, Vec<TxIdx>)>,
    ) -> Self {
        let data = DashMap::default();
        // We preallocate estimated locations to avoid restructuring trees at runtime
        // while holding a write lock. Ideally [dashmap] would have a lock-free
        // construction API. This is acceptable for now as it's a non-congested one-time
        // cost.
        for (location_hash, estimated_tx_idxs) in estimated_locations {
            data.insert(
                location_hash,
                estimated_tx_idxs
                    .into_iter()
                    .map(|tx_idx| (tx_idx, MemoryEntry::Estimate))
                    .collect(),
            );
        }
        Self {
            data,
            last_locations: (0..block_size).map(|_| Mutex::default()).collect(),
        }
    }

    // Apply a new pair of read & write sets to the multi-version data structure.
    // Return whether a write occurred to a memory location not written to by
    // the previous incarnation of the same transaction. This determines whether
    // the executed higher transactions need re-validation.
    pub(crate) fn record(
        &self,
        tx_version: &TxVersion,
        read_locations: ReadLocations,
        write_set: WriteSet,
    ) -> bool {
        // Update the multi-version as fast as possible for higher transactions to
        // read from.
        let new_locations: Vec<MemoryLocationHash> = write_set.iter().map(|(l, _)| *l).collect();
        for (location, value) in write_set.into_iter() {
            let entry = MemoryEntry::Data(tx_version.tx_incarnation, value);
            // We must not use [get_mut] here, else there's a race condition where
            // two threads get [None] first, then the latter's [insert] overwrote
            // the former's.
            match self.data.entry(location) {
                Entry::Occupied(mut written_transactions) => {
                    written_transactions
                        .get_mut()
                        .insert(tx_version.tx_idx, entry);
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(BTreeMap::from([(tx_version.tx_idx, entry)]));
                }
            }
        }
        // TODO: Faster "difference" function when there are many locations
        let mut last_locations = index_mutex!(self.last_locations, tx_version.tx_idx);
        for prev_location in last_locations.write.iter() {
            if !new_locations.contains(prev_location) {
                if let Some(mut written_transactions) = self.data.get_mut(prev_location) {
                    written_transactions.remove(&tx_version.tx_idx);
                }
            }
        }

        // Update this transaction's read & write set for the next validation.
        last_locations.read = read_locations;
        for new_location in new_locations.iter() {
            if !last_locations.write.contains(new_location) {
                // We update right before returning to avoid an early clone.
                last_locations.write = new_locations;
                return true;
            }
        }
        // We update right before returning to avoid an early clone.
        last_locations.write = new_locations;
        false
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
        for (location, prior_origins) in index_mutex!(self.last_locations, tx_idx).read.iter() {
            if let Some(written_transactions) = self.read_location(location) {
                let mut iter = written_transactions.range(..tx_idx);
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
                        // The previously read value is now cleared
                        // or marked with ESTIMATE.
                        else {
                            return false;
                        }
                    }
                    // Read from storage but there is now something
                    // in between!
                    else if iter.next_back().is_some() {
                        return false;
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
        // TODO: Better error handling
        for location in index_mutex!(self.last_locations, tx_idx).write.iter() {
            if let Some(mut written_transactions) = self.data.get_mut(location) {
                written_transactions.insert(tx_idx, MemoryEntry::Estimate);
            }
        }
    }

    pub(crate) fn read_location(
        &self,
        location: &MemoryLocationHash,
    ) -> Option<Ref<MemoryLocationHash, BTreeMap<TxIdx, MemoryEntry>>> {
        self.data.get(location)
    }

    pub(crate) fn consume_location(
        &self,
        location: &MemoryLocationHash,
    ) -> Option<BTreeMap<TxIdx, MemoryEntry>> {
        let (_, tree) = self.data.remove(location)?;
        Some(tree)
    }
}
