// Copyright 2019 Cargill Incorporated
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

//! Additional behaviors surrounding cloneable structs.

use std::sync::Mutex;

/// A CloneVat is a shareable source for a cloneable struct.
///
/// This struct removes the needs for an underlying clonable object to have to be sync as well,
/// This has value in that the vat instance can be passed among threads and provide instances to
/// a running thread.
///
/// # Note
///
/// This should be used for structs that are not Sync (i.e. `!Sync for T`), but it is not yet
/// possible to create the appropriate trait bounds in stable Rust. In other words, objects that
/// are Sync should just be used by themselves.
pub struct CloneVat<T: Clone> {
    progenitor: Mutex<T>,
}

impl<T: Clone> CloneVat<T> {
    /// Construct a new clone vat with the given source.
    pub fn new(progenitor: T) -> Self {
        Self {
            progenitor: Mutex::new(progenitor),
        }
    }

    /// Returns a new clone from the vat.
    pub fn get_clone(&self) -> T {
        match self.progenitor.lock() {
            Ok(source) => source.clone(),
            // This is safe to do when the progenitor is !Sync (its interior state could not have
            // been mutated by the other thread).
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}
