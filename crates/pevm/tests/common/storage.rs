use hashbrown::HashMap;
use revm::primitives::{
    alloy_primitives::U160, keccak256, ruint::UintTryFrom, Address, B256, I256, U256,
};
use rustc_hash::FxBuildHasher;

/// A builder for constructing storage mappings, using `U256` keys and values.
#[derive(Debug, Default)]
pub struct StorageBuilder {
    dict: HashMap<U256, U256, FxBuildHasher>,
}

impl StorageBuilder {
    /// Creates a new, empty `StorageBuilder` instance.
    pub fn new() -> Self {
        Self {
            dict: HashMap::default(),
        }
    }

    /// Inserts a key-value pair into the storage builder's dictionary.
    pub fn set<K, V>(&mut self, slot: K, value: V)
    where
        U256: UintTryFrom<K> + UintTryFrom<V>,
    {
        self.dict.insert(U256::from(slot), U256::from(value));
    }

    /// Inserts multiple key-value pairs into the storage builder's dictionary.
    pub fn set_many<K: Copy, const L: usize>(&mut self, starting_slot: K, value: &[U256; L])
    where
        U256: UintTryFrom<K> + UintTryFrom<usize>,
    {
        for (index, item) in value.iter().enumerate() {
            let slot = U256::from(starting_slot).wrapping_add(U256::from(index));
            self.dict.insert(slot, *item);
        }
    }

    /// Sets a value in the storage builder's dictionary at a specified offset within an existing entry.
    pub fn set_with_offset<K: Copy, V>(&mut self, key: K, offset: usize, length: usize, value: V)
    where
        U256: UintTryFrom<K> + UintTryFrom<V>,
    {
        let entry = self.dict.entry(U256::from(key)).or_default();
        let mut buffer = B256::from(*entry);
        let value_buffer = B256::from(U256::from(value));
        buffer[(32 - offset - length)..(32 - offset)]
            .copy_from_slice(&value_buffer[(32 - length)..32]);
        *entry = buffer.into();
    }

    /// Returns the constructed `HashMap` from the storage builder.
    pub fn build(self) -> HashMap<U256, U256, FxBuildHasher> {
        self.dict
    }
}

/// Converts an `Address` (20-byte Ethereum address) into a `U256` value.
pub fn from_address(address: Address) -> U256 {
    let encoded_as_u160: U160 = address.into();
    U256::from(encoded_as_u160)
}

/// Converts a short string into a `U256` value, encoding its contents with specific padding logic.
pub fn from_short_string(text: &str) -> U256 {
    assert!(text.len() < 32);
    let encoded_as_b256 = B256::bit_or(
        B256::right_padding_from(text.as_bytes()),
        B256::left_padding_from(&[(text.len() * 2) as u8]),
    );
    encoded_as_b256.into()
}

/// Generates a unique `U256` hash based on a slot and a sequence of indices.
pub fn from_indices<K, V: Copy>(slot: K, indices: &[V]) -> U256
where
    U256: UintTryFrom<K> + UintTryFrom<V>,
{
    let mut result = B256::from(U256::from(slot));
    for index in indices {
        let to_prepend = B256::from(U256::from(*index));
        result = keccak256([to_prepend.as_slice(), result.as_slice()].concat())
    }
    result.into()
}

/// Converts a tick value represented as a signed 32-bit integer (`i32`) to an unsigned 256-bit integer (`U256`).
pub fn from_tick(tick: i32) -> U256 {
    let encoded_as_i256 = I256::try_from(tick).unwrap();
    encoded_as_i256.into_raw()
}
