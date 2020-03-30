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

//! Implementation of generic traits that are useful for the storage vector.

use super::{
    Iter,
    IterMut,
    Vec as StorageVec,
};
use crate::storage2::{
    PullForward,
    SaturatingStorage,
    StorageFootprint,
};
use core::iter::{
    Extend,
    FromIterator,
};

impl<T> core::ops::Index<u32> for StorageVec<T>
where
    T: StorageFootprint + PullForward,
{
    type Output = T;

    fn index(&self, index: u32) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> core::ops::IndexMut<u32> for StorageVec<T>
where
    T: SaturatingStorage + StorageFootprint + PullForward,
{
    fn index_mut(&mut self, index: u32) -> &mut Self::Output {
        self.get_mut(index).expect("index out of bounds")
    }
}

impl<'a, T: 'a> IntoIterator for &'a StorageVec<T>
where
    T: StorageFootprint + PullForward,
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: 'a> IntoIterator for &'a mut StorageVec<T>
where
    T: StorageFootprint + SaturatingStorage + PullForward,
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> Extend<T> for StorageVec<T>
where
    T: StorageFootprint + SaturatingStorage,
{
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for item in iter {
            self.push(item)
        }
    }
}

impl<T> FromIterator<T> for StorageVec<T>
where
    T: StorageFootprint + SaturatingStorage,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut vec = StorageVec::new();
        vec.extend(iter);
        vec
    }
}

impl<T> core::cmp::PartialEq for StorageVec<T>
where
    T: PartialEq + StorageFootprint + PullForward,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false
        }
        self.iter().zip(other.iter()).all(|(lhs, rhs)| lhs == rhs)
    }
}

impl<T> core::cmp::Eq for StorageVec<T> where T: Eq + StorageFootprint + PullForward {}