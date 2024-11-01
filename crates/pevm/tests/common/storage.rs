use hashbrown::HashMap;
use revm::primitives::{
    alloy_primitives::U160, keccak256, ruint::UintTryFrom, Address, B256, I256, U256,
};
use rustc_hash::FxBuildHasher;

#[derive(Debug, Default)]
pub struct StorageBuilder {
    dict: HashMap<U256, U256, FxBuildHasher>,
}

impl StorageBuilder {
    pub fn new() -> Self {
        Self {
            dict: HashMap::default(),
        }
    }

    pub fn set<K, V>(&mut self, slot: K, value: V)
    where
        U256: UintTryFrom<K>,
        U256: UintTryFrom<V>,
    {
        self.dict.insert(U256::from(slot), U256::from(value));
    }

    pub fn set_many<K: Copy, const L: usize>(&mut self, starting_slot: K, value: &[U256; L])
    where
        U256: UintTryFrom<K>,
        U256: UintTryFrom<usize>,
    {
        for (index, item) in value.iter().enumerate() {
            let slot = U256::from(starting_slot).wrapping_add(U256::from(index));
            self.dict.insert(slot, *item);
        }
    }

    pub fn set_with_offset<K: Copy, V>(&mut self, key: K, offset: usize, length: usize, value: V)
    where
        U256: UintTryFrom<K>,
        U256: UintTryFrom<V>,
    {
        let entry = self.dict.entry(U256::from(key)).or_default();
        let mut buffer = B256::from(*entry);
        let value_buffer = B256::from(U256::from(value));
        buffer[(32 - offset - length)..(32 - offset)]
            .copy_from_slice(&value_buffer[(32 - length)..32]);
        *entry = buffer.into();
    }

    pub fn build(self) -> HashMap<U256, U256, FxBuildHasher> {
        self.dict
    }
}

pub fn from_address(address: Address) -> U256 {
    let encoded_as_u160: U160 = address.into();
    U256::from(encoded_as_u160)
}

pub fn from_short_string(text: &str) -> U256 {
    assert!(text.len() < 32);
    let encoded_as_b256 = B256::bit_or(
        B256::right_padding_from(text.as_bytes()),
        B256::left_padding_from(&[(text.len() * 2) as u8]),
    );
    encoded_as_b256.into()
}

pub fn from_indices<K, V: Copy>(slot: K, indices: &[V]) -> U256
where
    U256: UintTryFrom<K>,
    U256: UintTryFrom<V>,
{
    let mut result = B256::from(U256::from(slot));
    for index in indices {
        let to_prepend = B256::from(U256::from(*index));
        result = keccak256([to_prepend.as_slice(), result.as_slice()].concat())
    }
    result.into()
}

pub fn from_tick(tick: i32) -> U256 {
    let encoded_as_i256 = I256::try_from(tick).unwrap();
    encoded_as_i256.into_raw()
}
