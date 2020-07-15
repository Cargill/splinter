// Copyright 2018-2020 Cargill Incorporated
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

use std::error::Error;
use std::fmt;

/// Represents KeyStore errors
#[derive(Debug)]
pub enum KeyStoreError {
    /// Represents CRUD operations failures
    OperationError {
        context: String,
        source: Box<dyn Error>,
    },
    /// Represents database query failures
    QueryError {
        context: String,
        source: Box<dyn Error>,
    },
    /// Represents general failures in the database
    StorageError {
        context: String,
        source: Option<Box<dyn Error>>,
    },
    /// Represents an issue connecting to the database
    ConnectionError(Box<dyn Error>),
    /// Returned when a key is not found by the provided ID
    NotFoundError(String),
    /// Returned when a key with the same ID is already in the database
    DuplicateKeyError(String),
    /// Returned when a user is not found with the provided ID
    UserDoesNotExistError(String),
}

impl Error for KeyStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            KeyStoreError::OperationError { source, .. } => Some(&**source),
            KeyStoreError::QueryError { source, .. } => Some(&**source),
            KeyStoreError::StorageError {
                source: Some(source),
                ..
            } => Some(&**source),
            KeyStoreError::StorageError { source: None, .. } => None,
            KeyStoreError::ConnectionError(err) => Some(&**err),
            KeyStoreError::NotFoundError(_) => None,
            KeyStoreError::DuplicateKeyError(_) => None,
            KeyStoreError::UserDoesNotExistError(_) => None,
        }
    }
}

impl fmt::Display for KeyStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyStoreError::OperationError { context, source } => {
                write!(f, "failed to perform operation: {}: {}", context, source)
            }
            KeyStoreError::QueryError { context, source } => {
                write!(f, "failed query: {}: {}", context, source)
            }
            KeyStoreError::StorageError {
                context,
                source: Some(source),
            } => write!(
                f,
                "the underlying storage returned an error: {}: {}",
                context, source
            ),
            KeyStoreError::StorageError {
                context,
                source: None,
            } => write!(f, "the underlying storage returned an error: {}", context),
            KeyStoreError::ConnectionError(err) => {
                write!(f, "failed to connect to underlying storage: {}", err)
            }
            KeyStoreError::NotFoundError(msg) => write!(f, "key not found: {}", msg),
            KeyStoreError::DuplicateKeyError(msg) => write!(f, "key already exists: {}", msg),
            KeyStoreError::UserDoesNotExistError(msg) => write!(f, "user does not exist: {}", msg),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for KeyStoreError {
    fn from(err: diesel::r2d2::PoolError) -> KeyStoreError {
        KeyStoreError::ConnectionError(Box::new(err))
    }
}
