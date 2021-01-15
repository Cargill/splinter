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

use std::error::Error;
use std::fmt;

#[cfg(feature = "diesel")]
use crate::database::error;

/// Represents UserStore errors
#[derive(Debug)]
pub enum UserStoreError {
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
    NotFoundError(String),
}

impl Error for UserStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            UserStoreError::OperationError { source, .. } => Some(&**source),
            UserStoreError::QueryError { source, .. } => Some(&**source),
            UserStoreError::StorageError {
                source: Some(source),
                ..
            } => Some(&**source),
            UserStoreError::StorageError { source: None, .. } => None,
            UserStoreError::ConnectionError(err) => Some(&**err),
            UserStoreError::NotFoundError(_) => None,
        }
    }
}

impl fmt::Display for UserStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserStoreError::OperationError { context, source } => {
                write!(f, "failed to perform operation: {}: {}", context, source)
            }
            UserStoreError::QueryError { context, source } => {
                write!(f, "failed query: {}: {}", context, source)
            }
            UserStoreError::StorageError {
                context,
                source: Some(source),
            } => write!(
                f,
                "the underlying storage returned an error: {}: {}",
                context, source
            ),
            UserStoreError::StorageError {
                context,
                source: None,
            } => write!(f, "the underlying storage returned an error: {}", context),
            UserStoreError::ConnectionError(err) => {
                write!(f, "failed to connect to underlying storage: {}", err)
            }
            UserStoreError::NotFoundError(ref s) => write!(f, "User not found: {}", s),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<error::ConnectionError> for UserStoreError {
    fn from(err: error::ConnectionError) -> UserStoreError {
        UserStoreError::ConnectionError(Box::new(err))
    }
}
