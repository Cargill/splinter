/*
 * Copyright 2019 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */
use std::error::Error;
use std::fmt;

use super::super::error::ModelConversionError;
use crate::database::error::DatabaseError;

#[derive(Debug)]
pub enum NotificationManagerError {
    OperationError(String),
    QueryError(String),
    StorageError(String),
    ConnectionError(String),
    ConversionError(String),
}

impl Error for NotificationManagerError {
    fn description(&self) -> &str {
        match *self {
            NotificationManagerError::OperationError(ref msg) => msg,
            NotificationManagerError::QueryError(ref msg) => msg,
            NotificationManagerError::StorageError(ref msg) => msg,
            NotificationManagerError::ConnectionError(ref msg) => msg,
            NotificationManagerError::ConversionError(ref msg) => msg,
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            NotificationManagerError::OperationError(_) => None,
            NotificationManagerError::QueryError(_) => None,
            NotificationManagerError::StorageError(_) => None,
            NotificationManagerError::ConnectionError(_) => None,
            NotificationManagerError::ConversionError(_) => None,
        }
    }
}

impl fmt::Display for NotificationManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NotificationManagerError::OperationError(ref s) => {
                write!(f, "failed to perform operation: {}", s)
            }
            NotificationManagerError::QueryError(ref s) => write!(f, "failed query: {}", s),
            NotificationManagerError::StorageError(ref s) => {
                write!(f, "the underlying storage returned an error: {}", s)
            }
            NotificationManagerError::ConnectionError(ref s) => {
                write!(f, "failed to connect to underlying storage: {}", s)
            }
            NotificationManagerError::ConversionError(ref s) => {
                write!(f, "conversion failed: {}", s)
            }
        }
    }
}

impl From<DatabaseError> for NotificationManagerError {
    fn from(err: DatabaseError) -> NotificationManagerError {
        match err {
            DatabaseError::ConnectionError(err) => {
                NotificationManagerError::ConnectionError(format!("{}", err))
            }
            _ => NotificationManagerError::StorageError(format!("{}", err)),
        }
    }
}

impl From<ModelConversionError> for NotificationManagerError {
    fn from(err: ModelConversionError) -> NotificationManagerError {
        NotificationManagerError::ConversionError(format!("{}", err))
    }
}
