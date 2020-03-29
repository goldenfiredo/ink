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

mod impls;
mod iter;
mod storage;

#[cfg(test)]
mod tests;

pub use self::iter::{
    Iter,
    IterMut,
};
use crate::storage2::{
    LazyChunk,
    Pack,
    PullForward,
    StorageFootprint,
};
use ink_primitives::Key;

/// An index into the stash.
type Index = u32;

#[derive(Debug)]
pub struct Stash<T> {
    /// The combined and commonly used header data.
    header: Pack<Header>,
    /// The storage entries of the stash.
    entries: LazyChunk<Pack<Entry<T>>>,
}

/// Stores general commonly required information about the storage stash.
#[derive(Debug, scale::Encode, scale::Decode)]
pub struct Header {
    /// The latest vacant index.
    ///
    /// - If all entries are occupied:
    ///     - Points to the entry at index `self.len`.
    /// - If some entries are vacant:
    ///     - Points to the entry that has been vacated most recently.
    last_vacant: Index,
    /// The number of items stored in the stash.
    ///
    /// # Note
    ///
    /// We cannot simply use the underlying length of the vector
    /// since it would include vacant slots as well.
    len: u32,
    /// The number of entries currently managed by the stash.
    len_entries: u32,
}

/// A vacant entry with previous and next vacant indices.
#[derive(Debug, Copy, Clone, scale::Encode, scale::Decode)]
pub struct VacantEntry {
    /// The next vacant index.
    next: Index,
    /// The previous vacant index.
    prev: Index,
}

/// An entry within the stash.
///
/// The vacant entries within a storage stash form a doubly linked list of
/// vacant entries that is used to quickly re-use their vacant storage.
#[derive(Debug, scale::Encode, scale::Decode)]
pub enum Entry<T> {
    /// A vacant entry that holds the index to the next and previous vacant entry.
    Vacant(VacantEntry),
    /// An occupied entry that hold the value.
    Occupied(T),
}

impl<T> Entry<T> {
    /// Returns `true` if the entry is occupied.
    pub fn is_occupied(&self) -> bool {
        if let Entry::Occupied(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if the entry is vacant.
    pub fn is_vacant(&self) -> bool {
        !self.is_occupied()
    }

    /// Returns the vacant entry if the entry is vacant, otherwise returns `None`.
    fn try_to_vacant(&self) -> Option<VacantEntry> {
        match self {
            Entry::Occupied(_) => None,
            Entry::Vacant(vacant_entry) => Some(*vacant_entry),
        }
    }

    /// Returns the vacant entry if the entry is vacant, otherwise returns `None`.
    fn try_to_vacant_mut(&mut self) -> Option<&mut VacantEntry> {
        match self {
            Entry::Occupied(_) => None,
            Entry::Vacant(vacant_entry) => Some(vacant_entry),
        }
    }
}

impl<T> Stash<T> {
    /// Creates a new empty stash.
    pub fn new() -> Self {
        Self {
            header: Pack::new(Header {
                last_vacant: 0,
                len: 0,
                len_entries: 0,
            }),
            entries: LazyChunk::new(),
        }
    }

    /// Returns the number of elements stored in the stash.
    pub fn len(&self) -> u32 {
        self.header.len
    }

    /// Returns `true` if the stash contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of entries currently managed by the storage stash.
    fn len_entries(&self) -> u32 {
        self.header.len_entries
    }

    /// Returns the underlying key to the cells.
    ///
    /// # Note
    ///
    /// This is a low-level utility getter and should
    /// normally not be required by users.
    pub fn entries_key(&self) -> Option<&Key> {
        self.entries.key()
    }

    /// Returns an iterator yielding shared references to all elements of the stash.
    ///
    /// # Note
    ///
    /// Avoid unbounded iteration over big storage stashs.
    /// Prefer using methods like `Iterator::take` in order to limit the number
    /// of yielded elements.
    pub fn iter(&self) -> Iter<T> {
        Iter::new(self)
    }

    /// Returns an iterator yielding exclusive references to all elements of the stash.
    ///
    /// # Note
    ///
    /// Avoid unbounded iteration over big storage stashs.
    /// Prefer using methods like `Iterator::take` in order to limit the number
    /// of yielded elements.
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut::new(self)
    }

    /// Returns `true` if the storage stash has vacant entries.
    fn has_vacant_entries(&self) -> bool {
        self.header.last_vacant != self.header.len_entries
    }

    /// Returns the index of the last vacant entry if any.
    fn last_vacant_index(&self) -> Option<Index> {
        if self.has_vacant_entries() {
            Some(self.header.last_vacant)
        } else {
            None
        }
    }
}

impl<T> Stash<T>
where
    T: scale::Decode + StorageFootprint + PullForward,
{
    /// Returns a shared reference to the element at the given index.
    pub fn get(&self, at: Index) -> Option<&T> {
        if at >= self.len_entries() {
            // Bail out early if the index is out of bounds.
            return None
        }
        self.entries.get(at).and_then(|entry| {
            match Pack::as_inner(entry) {
                Entry::Occupied(val) => Some(val),
                Entry::Vacant { .. } => None,
            }
        })
    }

    /// Returns an exclusive reference to the element at the given index.
    pub fn get_mut(&mut self, at: Index) -> Option<&mut T> {
        if at >= self.len_entries() {
            // Bail out early if the index is out of bounds.
            return None
        }
        self.entries.get_mut(at).and_then(|entry| {
            match Pack::as_inner_mut(entry) {
                Entry::Occupied(val) => Some(val),
                Entry::Vacant { .. } => None,
            }
        })
    }
}

impl<T> Stash<T>
where
    T: scale::Codec + StorageFootprint + PullForward,
{
    /// Rebinds the `prev` and `next` bindings of the neighbours of the vacant entry.
    ///
    /// # Note
    ///
    /// The `removed_index` points to the index of the removed vacant entry.
    fn remove_vacant_entry(&mut self, removed_index: Index, vacant_entry: VacantEntry) {
        let prev_vacant = vacant_entry.prev;
        let next_vacant = vacant_entry.next;
        if prev_vacant == removed_index && next_vacant == removed_index {
            // There is no other vacant entry left in the storage stash so
            // there is nothing to update. Bail out early.
            return
        }
        if prev_vacant == next_vacant {
            // There is only one other vacant entry left.
            // We can update the single vacant entry in a single look-up.
            self.entries
                .get_mut(prev_vacant)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`prev` must point to an existing entry at this point")
                .map(|entry| {
                    entry.prev = prev_vacant;
                    entry.next = prev_vacant;
                });
        } else {
            // There are multiple other vacant entries left.
            self.entries
                .get_mut(prev_vacant)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`prev` must point to an existing entry at this point")
                .map(|entry| {
                    entry.next = next_vacant;
                });
            self.entries
                .get_mut(next_vacant)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`next` must point to an existing entry at this point")
                .map(|entry| {
                    entry.prev = prev_vacant;
                });
        }
        // Bind the last vacant pointer to the vacant position with the lower index.
        // This has the effect that lower indices are refilled more quickly.
        self.header.last_vacant = core::cmp::min(prev_vacant, next_vacant);
    }

    /// Put the element into the stash at the next vacant position.
    ///
    /// Returns the stash index that the element was put into.
    pub fn put(&mut self, new_value: T) -> Index {
        let new_entry = Some(Pack::new(Entry::Occupied(new_value)));
        let new_index = if let Some(index) = self.last_vacant_index() {
            // Put the new element to the most recent vacant index if not all entries are occupied.
            let old_entry = self
                .entries
                .put_get(index, new_entry)
                .expect("a `next_vacant` index must point to an occupied cell");
            let vacant_entry = match Pack::into_inner(old_entry) {
                Entry::Vacant(vacant_entry) => vacant_entry,
                Entry::Occupied(_) => {
                    unreachable!("next_vacant must point to a vacant entry")
                }
            };
            self.remove_vacant_entry(index, vacant_entry);
            index
        } else {
            // Push the new element to the end if all entries are occupied.
            self.entries.put(self.header.len_entries, new_entry);
            self.header.last_vacant += 1;
            self.header.len_entries += 1;
            self.header.len_entries
        };
        self.header.len += 1;
        new_index
    }

    /// Takes the element stored at the given index if any.
    pub fn take(&mut self, at: Index) -> Option<T> {
        // Cases:
        // - There are vacant entries already.
        // - There are no vacant entries before.
        if at >= self.len() {
            // Early return since `at` index is out of bounds.
            return None
        }
        // Precompute prev and next vacant entires as we might need them later.
        // Due to borrow checker constraints we cannot have this at a later stage.
        let (prev, next) = if let Some(index) = self.last_vacant_index() {
            self.entries
                .get(index)
                .expect("last_vacant must point to an existing entry")
                .try_to_vacant()
                .map(|vacant_entry| (vacant_entry.prev, vacant_entry.next))
                .expect("last_vacant must point to a vacant entry")
        } else {
            // Default prev and next to the given at index.
            // So the resulting vacant index is pointing to itself.
            (at, at)
        };
        let entry_mut =
            Pack::as_inner_mut(self.entries.get_mut(at).expect("index is within bounds"));
        if entry_mut.is_vacant() {
            // Early return if the taken entry is already vacant.
            return None
        }
        // At this point we know that the entry is occupied with a value.
        let new_vacant_entry = Entry::Vacant(VacantEntry { prev, next });
        let taken_entry = core::mem::replace(entry_mut, new_vacant_entry);
        // Update links from and to neighbouring vacant entries.
        if prev == next {
            // Previous and next are the same so we can update the vacant
            // neighbour with a single look-up.
            self.entries
                .get_mut(next)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`next` must point to an existing entry at this point")
                .map(|entry| {
                    entry.prev = at;
                    entry.next = at;
                });
        } else {
            // Previous and next vacant entries are different and thus need
            // different look-ups to update them.
            self.entries
                .get_mut(prev)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`prev` must point to an existing entry at this point")
                .map(|entry| {
                    entry.next = at;
                });
            self.entries
                .get_mut(next)
                .map(Pack::as_inner_mut)
                .map(Entry::try_to_vacant_mut)
                .expect("`next` must point to an existing entry at this point")
                .map(|entry| {
                    entry.prev = at;
                });
        }
        // Take the value out of the taken occupied entry and return it.
        match taken_entry {
            Entry::Occupied(value) => {
                use core::cmp::min;
                self.header.last_vacant = min(at, min(prev, next));
                self.header.len -= 1;
                Some(value)
            }
            Entry::Vacant { .. } => {
                unreachable!("the taken entry is known to be occupied")
            }
        }
    }

    /// Defragments the underlying storage to minimize footprint.
    ///
    /// This might invalidate indices stored outside of the stash.
    ///
    /// # Callback
    ///
    /// In order to keep those indices up-to-date the caller can provide
    /// a callback function that is called for every moved entry
    /// with a shared reference to the entries value and the old as well
    /// as the new index.
    ///
    /// # Note
    ///
    /// - If `max_iterations` is `Some` concrete value it is used in order to
    ///   bound the number of iterations and won't try to defrag until the stash
    ///   is optimally compacted.
    /// - Users are adviced to call this method using `Some` concrete
    ///   value to keep gas costs within certain bounds.
    /// - The call to the given callback takes place before the reinsertion
    ///   of the shifted occupied entry.
    pub fn defrag<C>(&mut self, max_iterations: Option<u32>, mut callback: C)
    where
        C: FnMut(Index, Index, &T),
    {
        let len_entries = self.len_entries();
        for index in (0..len_entries)
            .rev()
            .take(max_iterations.unwrap_or(len_entries) as usize)
        {
            if !self.has_vacant_entries() {
                // Bail out as soon as there are no more vacant entries left.
                return
            }
            match Pack::into_inner(
                self.entries.take(index).expect("index is within bounds"),
            ) {
                Entry::Vacant(vacant_entry) => {
                    // Remove the vacant entry and rebind its neighbours.
                    self.remove_vacant_entry(index, vacant_entry);
                }
                Entry::Occupied(value) => {
                    // Move the occupied entry into one of the remaining vacant
                    // entries. We do not re-use the `put` method to not update
                    // the length and other header information.
                    let vacant_index = self
                    .last_vacant_index()
                    .expect("it has been asserted that there are vacant entries");
                    callback(index, vacant_index, &value);
                    let new_entry = Some(Pack::new(Entry::Occupied(value)));
                    let old_entry = self
                        .entries
                        .put_get(vacant_index, new_entry)
                        .expect("a `next_vacant` index must point to an occupied cell");
                    let vacant_entry = match Pack::into_inner(old_entry) {
                        Entry::Vacant(vacant_entry) => vacant_entry,
                        Entry::Occupied(_) => {
                            unreachable!("next_vacant must point to a vacant entry")
                        }
                    };
                    self.remove_vacant_entry(index, vacant_entry);
                }
            }
            self.header.len_entries -= 1;
        }
    }
}