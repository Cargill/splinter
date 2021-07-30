/*
 * Copyright 2018-2020 Cargill Incorporated
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

use cylinder::{KeyParseError, SigningError};
use sabre_sdk::protocol::{
    payload::{ActionBuildError, SabrePayloadBuildError},
    AddressingError,
};
use splinter::events;
use transact::{
    protocol::{batch::BatchBuildError, transaction::TransactionBuildError},
    protos::ProtoConversionError,
};

use crate::application_metadata::ApplicationMetadataError;

#[derive(Debug)]
pub enum AppAuthHandlerError {
    Io(std::io::Error),
    InvalidMessage(String),
    Database(String),
    Reactor(events::ReactorError),
    WebSocket(events::WebSocketError),
    Sabre(String),
    Signing(String),
    Transact(String),
    BatchSubmit(String),
}

impl Error for AppAuthHandlerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppAuthHandlerError::Io(err) => Some(err),
            AppAuthHandlerError::InvalidMessage(_) => None,
            AppAuthHandlerError::Database(_) => None,
            AppAuthHandlerError::Reactor(err) => Some(err),
            AppAuthHandlerError::Sabre(_) => None,
            AppAuthHandlerError::Signing(_) => None,
            AppAuthHandlerError::Transact(_) => None,
            AppAuthHandlerError::BatchSubmit(_) => None,
            AppAuthHandlerError::WebSocket(err) => Some(err),
        }
    }
}

impl fmt::Display for AppAuthHandlerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppAuthHandlerError::Io(msg) => write!(f, "An I/O error occurred: {}", msg),
            AppAuthHandlerError::InvalidMessage(msg) => {
                write!(f, "The client received an invalid message: {}", msg)
            }
            AppAuthHandlerError::Database(msg) => {
                write!(f, "The database returned an error: {}", msg)
            }
            AppAuthHandlerError::Reactor(msg) => write!(f, "Reactor Error: {}", msg),
            AppAuthHandlerError::Sabre(msg) => write!(
                f,
                "An error occurred while building a Sabre payload: {}",
                msg
            ),
            AppAuthHandlerError::Signing(msg) => {
                write!(f, "A signing error occurred: {}", msg)
            }
            AppAuthHandlerError::Transact(msg) => write!(
                f,
                "An error occurred while building a transaction or batch: {}",
                msg
            ),
            AppAuthHandlerError::BatchSubmit(msg) => write!(
                f,
                "An error occurred while submitting a batch to the scabbard service: {}",
                msg
            ),
            AppAuthHandlerError::WebSocket(msg) => write!(f, "WebsocketError {}", msg),
        }
    }
}

impl From<std::io::Error> for AppAuthHandlerError {
    fn from(err: std::io::Error) -> AppAuthHandlerError {
        AppAuthHandlerError::Io(err)
    }
}

impl From<serde_json::error::Error> for AppAuthHandlerError {
    fn from(err: serde_json::error::Error) -> AppAuthHandlerError {
        AppAuthHandlerError::InvalidMessage(format!("{}", err))
    }
}

impl From<std::string::FromUtf8Error> for AppAuthHandlerError {
    fn from(err: std::string::FromUtf8Error) -> AppAuthHandlerError {
        AppAuthHandlerError::InvalidMessage(format!("{}", err))
    }
}

impl From<ApplicationMetadataError> for AppAuthHandlerError {
    fn from(err: ApplicationMetadataError) -> AppAuthHandlerError {
        AppAuthHandlerError::InvalidMessage(format!("{}", err))
    }
}

impl From<gameroom_database::DatabaseError> for AppAuthHandlerError {
    fn from(err: gameroom_database::DatabaseError) -> AppAuthHandlerError {
        AppAuthHandlerError::Database(format!("{}", err))
    }
}

impl From<diesel::result::Error> for AppAuthHandlerError {
    fn from(err: diesel::result::Error) -> Self {
        AppAuthHandlerError::Database(format!("Error performing query: {}", err))
    }
}

impl From<events::ReactorError> for AppAuthHandlerError {
    fn from(err: events::ReactorError) -> Self {
        AppAuthHandlerError::Reactor(err)
    }
}

impl From<events::WebSocketError> for AppAuthHandlerError {
    fn from(err: events::WebSocketError) -> Self {
        AppAuthHandlerError::WebSocket(err)
    }
}

macro_rules! impl_from_sabre_errors {
    ($($x:ty),*) => {
        $(
            impl From<$x> for AppAuthHandlerError {
                fn from(e: $x) -> Self {
                    AppAuthHandlerError::Sabre(e.to_string())
                }
            }
        )*
    };
}

impl_from_sabre_errors!(AddressingError, ActionBuildError, SabrePayloadBuildError);

impl From<KeyParseError> for AppAuthHandlerError {
    fn from(err: KeyParseError) -> Self {
        AppAuthHandlerError::Signing(err.to_string())
    }
}

impl From<SigningError> for AppAuthHandlerError {
    fn from(err: SigningError) -> Self {
        AppAuthHandlerError::Signing(err.to_string())
    }
}

impl From<BatchBuildError> for AppAuthHandlerError {
    fn from(err: BatchBuildError) -> Self {
        AppAuthHandlerError::Transact(err.to_string())
    }
}

impl From<TransactionBuildError> for AppAuthHandlerError {
    fn from(err: TransactionBuildError) -> Self {
        AppAuthHandlerError::Transact(err.to_string())
    }
}

impl From<ProtoConversionError> for AppAuthHandlerError {
    fn from(err: ProtoConversionError) -> Self {
        AppAuthHandlerError::Transact(err.to_string())
    }
}
