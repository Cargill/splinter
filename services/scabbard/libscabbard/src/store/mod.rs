// Copyright 2018-2021 Cargill Incorporated
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

//! Stores required for a scabbard services operation.

#[cfg(feature = "diesel")]
pub mod diesel;
mod error;
#[cfg(feature = "lmdb")]
pub mod lmdb;

pub use error::CommitHashStoreError;

/// A store for the current commit hash value.
///
/// The commit hash, for Scabbard's purposes is the current state root hash of the Merkle-Radix
/// tree after transactions have been applied.
pub trait CommitHashStore {
    /// Returns the current commit hash for the instance
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError>;

    /// Sets the current commit hash value.
    ///
    /// The commit hash, for Scabbard's purposes is the current state root hash of the Merkle-Radix
    /// tree after transactions have been applied.
    ///
    /// # Arguments
    ///
    /// * `current_commit_hash` - the new "current" commit hash.
    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError>;
}
