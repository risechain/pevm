use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A vector that allows for unsafe concurrent updates without locking.
/// The user must ensure that each index is accessed by only one thread at a time.
#[derive(Debug)]
pub(crate) struct UnsafeVec<T> {
    data: UnsafeCell<Vec<T>>,
    _marker: PhantomData<T>,
}

// Implementing Sync for UnsafeVec to allow sharing between threads.
unsafe impl<T> Sync for UnsafeVec<T> {}

impl<T> UnsafeVec<T> {
    pub(crate) fn new(vec: Vec<T>) -> UnsafeVec<T> {
        UnsafeVec {
            data: UnsafeCell::new(vec),
            _marker: PhantomData,
        }
    }

    /// Sets the value at the specified index.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it allows for concurrent mutable access to the vector.
    /// The caller must ensure that no other threads are accessing the same index concurrently.
    #[allow(dead_code)]
    pub(crate) fn set(&self, index: usize, value: T) {
        unsafe {
            (*self.data.get())[index] = value;
        }
    }

    /// Gets a reference to the value at the specified index.
    ///
    /// # Safety
    ///
    /// This method is unsafe for two reasons:
    ///
    /// 1. It allows for concurrent immutable access to the vector.
    ///    The caller must ensure that no other threads are mutating the same index concurrently.
    ///
    /// 2. The caller must ensure that the index is within the bounds of the vector.
    ///    Accessing an out-of-bounds index can lead to undefined behavior.
    pub(crate) fn get(&self, index: usize) -> &T {
        unsafe { (*self.data.get()).get_unchecked(index) }
    }

    /// Gets a mutable reference to the value at the specified index.
    ///
    /// # Safety
    ///
    /// This method is unsafe for two reasons:
    ///
    /// 1. It allows for concurrent mutable access to the vector.
    ///    The caller must ensure that no other threads are accessing the same index concurrently,
    ///    and that there are no overlapping mutable references to the same index.
    ///
    /// 2. The caller must ensure that the index is within the bounds of the vector.
    ///    Accessing an out-of-bounds index can lead to undefined behavior.
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn get_mut(&self, index: usize) -> &mut T {
        unsafe { (*self.data.get()).get_unchecked_mut(index) }
    }
}

// Implementing Deref to delegate method calls to the underlying vector.
impl<T> Deref for UnsafeVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data.get() }
    }
}

// Implementing DerefMut to delegate mutable method calls to the underlying vector.
impl<T> DerefMut for UnsafeVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data.get() }
    }
}
