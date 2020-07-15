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

#[derive(Debug)]
pub enum RefreshTokenError {
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

    // Represents the specific case where a query returns no records
    NotFoundError(String),
}

impl Error for RefreshTokenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RefreshTokenError::OperationError { source, .. } => Some(&**source),
            RefreshTokenError::QueryError { source, .. } => Some(&**source),
            RefreshTokenError::StorageError {
                source: Some(source),
                ..
            } => Some(&**source),
            RefreshTokenError::StorageError { source: None, .. } => None,
            RefreshTokenError::ConnectionError(err) => Some(&**err),
            RefreshTokenError::NotFoundError(_) => None,
        }
    }
}
impl fmt::Display for RefreshTokenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RefreshTokenError::OperationError { context, source } => {
                write!(f, "failed to perform operation: {}: {}", context, source)
            }
            RefreshTokenError::QueryError { context, source } => {
                write!(f, "failed query: {}: {}", context, source)
            }
            RefreshTokenError::StorageError {
                context,
                source: Some(source),
            } => write!(
                f,
                "the underlying storage returned an error: {}: {}",
                context, source
            ),
            RefreshTokenError::StorageError {
                context,
                source: None,
            } => write!(f, "the underlying storage returned an error: {}", context),
            RefreshTokenError::ConnectionError(ref s) => {
                write!(f, "failed to connect to underlying storage: {}", s)
            }
            RefreshTokenError::NotFoundError(ref s) => write!(f, "refresh token not found: {}", s),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for RefreshTokenError {
    fn from(err: diesel::r2d2::PoolError) -> RefreshTokenError {
        RefreshTokenError::ConnectionError(Box::new(err))
    }
}
