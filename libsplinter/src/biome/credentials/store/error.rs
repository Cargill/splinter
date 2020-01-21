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

use std::error::Error;
use std::fmt;

use crate::database::error::DatabaseError;

/// Represents CredentialsStore errors
#[derive(Debug)]
pub enum CredentialsStoreError {
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
        source: Box<dyn Error>,
    },
    /// Represents an issue connecting to the database
    ConnectionError(Box<dyn Error>),
    /// Represents error occured when an attempt is made to add a new credential with a
    /// username that already exists in the database
    DuplicateError(String),
    NotFoundError(String),
}

impl Error for CredentialsStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CredentialsStoreError::OperationError { source, .. } => Some(&**source),
            CredentialsStoreError::QueryError { source, .. } => Some(&**source),
            CredentialsStoreError::StorageError { source, .. } => Some(&**source),
            CredentialsStoreError::ConnectionError(err) => Some(&**err),
            CredentialsStoreError::DuplicateError(_) => None,
            CredentialsStoreError::NotFoundError(_) => None,
        }
    }
}
impl fmt::Display for CredentialsStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CredentialsStoreError::OperationError { context, source } => {
                write!(f, "failed to perform operation: {}: {}", context, source)
            }
            CredentialsStoreError::QueryError { context, source } => {
                write!(f, "failed query: {}: {}", context, source)
            }
            CredentialsStoreError::StorageError { context, source } => write!(
                f,
                "the underlying storage returned an error: {}: {}",
                context, source
            ),
            CredentialsStoreError::ConnectionError(ref s) => {
                write!(f, "failed to connect to underlying storage: {}", s)
            }
            CredentialsStoreError::DuplicateError(ref s) => {
                write!(f, "credentials already exists: {}", s)
            }
            CredentialsStoreError::NotFoundError(ref s) => {
                write!(f, "credentials not found: {}", s)
            }
        }
    }
}

impl From<DatabaseError> for CredentialsStoreError {
    fn from(err: DatabaseError) -> CredentialsStoreError {
        match err {
            DatabaseError::ConnectionError(_) => {
                CredentialsStoreError::ConnectionError(Box::new(err))
            }
            _ => CredentialsStoreError::StorageError {
                context: "The database returned an error".to_string(),
                source: Box::new(err),
            },
        }
    }
}
