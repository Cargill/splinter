// Copyright 2018-2022 Cargill Incorporated
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

//! Error types and logic for CommitHashStores.

use std::error::Error;
use std::fmt::Display;

use splinter::error::{
    InternalError, InvalidArgumentError, InvalidStateError, ResourceTemporarilyUnavailableError,
};

/// Error type for the [CommitHashStore](super::CommitHashStore) trait.
///
/// Any errors implementations of [CommitHashStore](super::CommitHashStore) can generate must be
/// convertible to a CommitHashStoreError enum member.

#[derive(Debug)]
/// Error states for fallible [CommitHashStore](super::CommitHashStore) operations.
pub enum CommitHashStoreError {
    Internal(InternalError),
    InvalidArgument(InvalidArgumentError),
    InvalidState(InvalidStateError),
    ResourceTemporarilyUnavailable(ResourceTemporarilyUnavailableError),
}

impl Display for CommitHashStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitHashStoreError::Internal(e) => e.fmt(f),
            CommitHashStoreError::InvalidArgument(e) => e.fmt(f),
            CommitHashStoreError::InvalidState(e) => e.fmt(f),
            CommitHashStoreError::ResourceTemporarilyUnavailable(e) => e.fmt(f),
        }
    }
}

impl Error for CommitHashStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CommitHashStoreError::Internal(e) => Some(e),
            CommitHashStoreError::InvalidArgument(e) => Some(e),
            CommitHashStoreError::InvalidState(e) => Some(e),
            CommitHashStoreError::ResourceTemporarilyUnavailable(e) => Some(e),
        }
    }
}

impl From<InternalError> for CommitHashStoreError {
    fn from(err: InternalError) -> Self {
        CommitHashStoreError::Internal(err)
    }
}

impl From<InvalidArgumentError> for CommitHashStoreError {
    fn from(err: InvalidArgumentError) -> Self {
        CommitHashStoreError::InvalidArgument(err)
    }
}

impl From<InvalidStateError> for CommitHashStoreError {
    fn from(err: InvalidStateError) -> Self {
        CommitHashStoreError::InvalidState(err)
    }
}
