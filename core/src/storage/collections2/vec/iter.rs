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

use crate::storage;

/// An iterator over the values of a storage `Vec`.
#[derive(Debug)]
pub struct Iter<'a, T> {
    /// The storage vector to iterate over.
    vec: &'a storage::Vec2<T>,
    /// The current begin of the iteration.
    begin: u32,
    /// The current end of the iteration.
    end: u32,
}

impl<'a, T> Iter<'a, T> {
    /// Creates a new iterator for the given storage vector.
    pub(crate) fn new(vec: &'a storage::Vec2<T>) -> Self {
        Self {
            vec,
            begin: 0,
            end: vec.len(),
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T>
where
    T: scale::Codec,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(self.begin <= self.end);
        if self.begin == self.end {
            return None
        }
        let cur = self.begin;
        self.begin += 1;
        self.vec.get(cur)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end - self.begin) as usize;
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> where T: scale::Codec {}

impl<'a, T> DoubleEndedIterator for Iter<'a, T>
where
    T: scale::Codec,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        debug_assert!(self.begin <= self.end);
        if self.begin == self.end {
            return None
        }
        debug_assert_ne!(self.end, 0);
        self.end -= 1;
        self.vec.get(self.end)
    }
}