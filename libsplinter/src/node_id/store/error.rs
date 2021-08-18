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

//! Error types and logic for NodeIdStores.

use std::convert::From;
use std::error::Error;
use std::fmt::Display;

use crate::error::InternalError;
use crate::error::ResourceTemporarilyUnavailableError;

/// Error type for the NodeIdStore trait.
/// Any errors implimentations of NodeIdStore can generate must be convertable
/// to a NodeIdStoreError enum member.

#[derive(Debug)]
/// Error states for fallible [NodeIdStore](../trait.NodeIdStore.html) operations.
pub enum NodeIdStoreError {
    InternalError(InternalError),
    ResourceTemporarilyUnavailableError(ResourceTemporarilyUnavailableError),
}

impl Display for NodeIdStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeIdStoreError::InternalError(e) => e.fmt(f),
            NodeIdStoreError::ResourceTemporarilyUnavailableError(e) => e.fmt(f),
        }
    }
}

impl Error for NodeIdStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodeIdStoreError::InternalError(e) => Some(e),
            NodeIdStoreError::ResourceTemporarilyUnavailableError(e) => Some(e),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::result::Error> for NodeIdStoreError {
    fn from(err: diesel::result::Error) -> Self {
        Self::InternalError(InternalError::from_source(Box::new(err)))
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for NodeIdStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        Self::ResourceTemporarilyUnavailableError(ResourceTemporarilyUnavailableError::from_source(
            Box::new(err),
        ))
    }
}

impl From<std::io::Error> for NodeIdStoreError {
    fn from(err: std::io::Error) -> Self {
        Self::InternalError(InternalError::from_source(Box::new(err)))
    }
}
