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

use bcrypt::BcryptError;

use std::error::Error;
use std::fmt;

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
        source: Option<Box<dyn Error>>,
    },

    /// Represents an issue connecting to the database
    ConnectionError(Box<dyn Error>),
    /// Represents error occured when an attempt is made to add a new credential with a
    /// username that already exists in the database
    DuplicateError(String),
    // Represents the specific case where a query returns no records
    NotFoundError(String),
}

impl Error for CredentialsStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CredentialsStoreError::OperationError { source, .. } => Some(&**source),
            CredentialsStoreError::QueryError { source, .. } => Some(&**source),
            CredentialsStoreError::StorageError {
                source: Some(source),
                ..
            } => Some(&**source),
            CredentialsStoreError::StorageError { source: None, .. } => None,
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
            CredentialsStoreError::StorageError {
                context,
                source: Some(source),
            } => write!(
                f,
                "the underlying storage returned an error: {}: {}",
                context, source
            ),
            CredentialsStoreError::StorageError {
                context,
                source: None,
            } => write!(f, "the underlying storage returned an error: {}", context,),
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

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for CredentialsStoreError {
    fn from(err: diesel::r2d2::PoolError) -> CredentialsStoreError {
        CredentialsStoreError::ConnectionError(Box::new(err))
    }
}

/// Represents CredentialsBuilder errors
#[derive(Debug)]
pub enum CredentialsBuilderError {
    /// Returned when a required field was not set
    MissingRequiredField(String),
    /// Returned when an error occurs building the credentials
    BuildError(Box<dyn Error>),
    /// Returned when an error occurs while attempting to encrypt the password
    EncryptionError(Box<dyn Error>),
}

impl Error for CredentialsBuilderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CredentialsBuilderError::MissingRequiredField(_) => None,
            CredentialsBuilderError::BuildError(err) => Some(&**err),
            CredentialsBuilderError::EncryptionError(err) => Some(&**err),
        }
    }
}

impl fmt::Display for CredentialsBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CredentialsBuilderError::MissingRequiredField(ref s) => {
                write!(f, "failed to build user credentials: {}", s)
            }
            CredentialsBuilderError::BuildError(ref s) => {
                write!(f, "failed to build credentials: {}", s)
            }
            CredentialsBuilderError::EncryptionError(ref s) => {
                write!(f, "failed encrypt password: {}", s)
            }
        }
    }
}

impl From<BcryptError> for CredentialsBuilderError {
    fn from(err: BcryptError) -> CredentialsBuilderError {
        CredentialsBuilderError::EncryptionError(Box::new(err))
    }
}

/// Represents Credentials errors
#[derive(Debug)]
pub enum CredentialsError {
    /// Returned when an error occurs while attempting to verify the password
    VerificationError(Box<dyn Error>),
}

impl Error for CredentialsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CredentialsError::VerificationError(err) => Some(&**err),
        }
    }
}

impl fmt::Display for CredentialsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CredentialsError::VerificationError(ref s) => {
                write!(f, "failed to verify password: {}", s)
            }
        }
    }
}

impl From<BcryptError> for CredentialsError {
    fn from(err: BcryptError) -> CredentialsError {
        CredentialsError::VerificationError(Box::new(err))
    }
}
