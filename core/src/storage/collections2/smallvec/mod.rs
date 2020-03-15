// Copyright 2019-2020 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod iter;
mod traits;

#[cfg(test)]
mod tests;

pub use self::iter::Iter;
use crate::{
    storage,
    storage::{
        LazyArray,
        LazyArrayLength,
        PullForward,
        SaturatingStorage,
        StorageFootprint,
    },
};

/// The used index type.
type Index = u32;

/// A contiguous growable array type.
///
/// # Note
///
/// - The `storage::SmallVec` has a very similar API compared to a `storage::Vec`.
///   The major difference between both data structures is that the `SmallVec`
///   can only contain up to a fixed amount of elements given by `N` whereas the
///   `Vec` can contain up to 2^32 elements which is the maximum for 32-bit Wasm
///   targets.
/// - The performance characteristics may be different from Rust's
///   `Vec` due to the internal differences.
/// - Allows to store up to N elements.
#[derive(Debug)]
pub struct SmallVec<T, N>
where
    N: LazyArrayLength<T>,
{
    /// The current length of the small vector.
    len: storage::Lazy<u32>,
    /// The entries of the small vector.
    elems: LazyArray<T, N>,
}

impl<T, N> Default for SmallVec<T, N>
where
    N: LazyArrayLength<T>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, N> SmallVec<T, N>
where
    N: LazyArrayLength<T>,
{
    /// Creates a new empty vector.
    pub fn new() -> Self {
        Self {
            len: storage::Lazy::new(0),
            elems: Default::default(),
        }
    }

    /// Returns the capacity of the small vector.
    pub fn capacity() -> u32 {
        <LazyArray<T, N>>::capacity()
    }

    /// Returns the number of elements in the vector, also referred to as its 'length'.
    pub fn len(&self) -> u32 {
        *self.len.get()
    }

    /// Returns `true` if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T, N> SmallVec<T, N>
where
    T: StorageFootprint + PullForward,
    N: LazyArrayLength<T>,
{
    /// Returns an iterator over the references of all elements stored in the vector.
    ///
    /// # Note
    ///
    /// - It is **not** recommended to iterate over all elements of a storage vector.
    /// - Try to avoid this if possible or iterate only over a minimal subset of
    ///   all elements using e.g. `Iterator::take(n)`.
    pub fn iter(&self) -> Iter<T, N> {
        Iter::new(self)
    }

    /// Returns the index if it is witihn bounds or `None` otherwise.
    fn within_bounds(&self, index: Index) -> Option<Index> {
        if index < self.len() {
            return Some(index)
        }
        None
    }

    /// Returns a shared reference to the first element if any.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Returns a shared reference to the last element if any.
    pub fn last(&self) -> Option<&T> {
        let last_index = self.len() - 1;
        self.get(last_index)
    }

    /// Returns a shared reference to the indexed element.
    ///
    /// Returns `None` if `index` is out of bounds.
    pub fn get(&self, index: u32) -> Option<&T> {
        self.within_bounds(index)
            .and_then(|index| self.elems.get(index))
    }
}

impl<T, N> SmallVec<T, N>
where
    T: StorageFootprint + SaturatingStorage,
    N: LazyArrayLength<T>,
{
    /// Appends an element to the back of the vector.
    pub fn push(&mut self, value: T) {
        assert!(
            self.len() < Self::capacity(),
            "cannot push more elements into the vector"
        );
        let last_index = self.len();
        *self.len += 1;
        self.elems.put(last_index, Some(value));
    }
}

impl<T, N> SmallVec<T, N>
where
    T: StorageFootprint + SaturatingStorage + PullForward,
    N: LazyArrayLength<T>,
{
    /// Pops the last element from the vector and returns it.
    //
    /// Returns `None` if the vector is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None
        }
        let last_index = self.len() - 1;
        *self.len = last_index;
        self.elems.take(last_index)
    }

    /// Pops the last element from the vector and immediately drops it.
    ///
    /// Does nothing if the vector is empty.
    ///
    /// # Note
    ///
    /// This operation is a bit more efficient than [`SmallVec::pop`] for some use cases.
    pub fn pop_drop(&mut self) {
        if self.is_empty() {
            return
        }
        let last_index = self.len() - 1;
        *self.len = last_index;
        self.elems.put(last_index, None);
    }

    /// Returns an exclusive reference to the first element if any.
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    /// Returns an exclusive reference to the last element if any.
    pub fn last_mut(&mut self) -> Option<&mut T> {
        let last_index = self.len() - 1;
        self.get_mut(last_index)
    }

    /// Returns an exclusive reference to the indexed element.
    ///
    /// Returns `None` if `index` is out of bounds.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        self.within_bounds(index)
            .and_then(move |index| self.elems.get_mut(index))
    }

    /// Swaps the elements at the given indices.
    ///
    /// # Panics
    ///
    /// If one or both indices are out of bounds.
    pub fn swap(&mut self, a: u32, b: u32) {
        self.elems.swap(a, b)
    }

    /// Removes the indexed element from the vector and returns it.
    ///
    /// The last element of the vector is put into the indexed slot.
    /// Returns `None` and does not mutate the vector if the index is out of bounds.
    ///
    /// # Note
    ///
    /// This operation does not preserve ordering but is constant time.
    pub fn swap_remove(&mut self, n: u32) -> Option<T> {
        if self.is_empty() {
            return None
        }
        self.elems.swap(n, self.len() - 1);
        self.pop()
    }

    /// Removes the indexed element from the vector.
    ///
    /// The last element of the vector is put into the indexed slot.
    /// Returns `Some(())` if an element has been removed and `None` otherwise.
    ///
    /// # Note
    ///
    /// This operation should be preferred over [`Vec::swap_remove`] if there is
    /// no need to return the removed element since it avoids a contract storage
    /// read for some use cases.
    pub fn swap_remove_drop(&mut self, n: u32) -> Option<()> {
        if self.is_empty() {
            return None
        }
        self.elems.put(n, None);
        let last_index = self.len() - 1;
        let last = self.elems.take(last_index);
        self.elems.put(n, last);
        *self.len = last_index;
        Some(())
    }
}