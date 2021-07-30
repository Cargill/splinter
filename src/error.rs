// Copyright 2018 Cargill Incorporated
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

use std::borrow::Borrow;
use std::error::Error as StdError;

use sabre_sdk::protocol::payload::{ActionBuildError, SabrePayloadBuildError};
use sabre_sdk::protos::ProtoConversionError;
use transact::{
    protocol::{batch::BatchBuildError, transaction::TransactionBuildError},
    protos::ProtoConversionError as TransactProtoConversionError,
};

#[derive(Debug)]
pub enum CliError {
    /// The user has provided invalid inputs; the string by this error
    /// is appropriate for display to the user without additional context
    User(String),
    Io(std::io::Error),
    Signing(String),
    Hyper(hyper::Error),
    ProtocolBuild(Box<dyn StdError>),
    ProtoConversion(ProtoConversionError),
    TransactProtoConversion(TransactProtoConversionError),
}

impl StdError for CliError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            CliError::User(_) => None,
            CliError::Io(err) => Some(err),
            CliError::Signing(_) => None,
            CliError::Hyper(err) => Some(err),
            CliError::ProtocolBuild(ref err) => Some(err.borrow()),
            CliError::ProtoConversion(err) => Some(err),
            CliError::TransactProtoConversion(err) => Some(err),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            CliError::User(ref s) => write!(f, "Error: {}", s),
            CliError::Io(ref err) => write!(f, "IoError: {}", err),
            CliError::Signing(ref msg) => write!(f, "SigningError: {}", msg),
            CliError::Hyper(ref err) => write!(f, "HyperError: {}", err),
            CliError::ProtocolBuild(ref err) => write!(f, "Protocol Error: {}", err),
            CliError::ProtoConversion(ref err) => write!(f, "Proto Conversion Error: {}", err),
            CliError::TransactProtoConversion(ref err) => {
                write!(f, "Transact Proto Conversion Error: {}", err)
            }
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}

impl From<hyper::Error> for CliError {
    fn from(e: hyper::Error) -> Self {
        CliError::Hyper(e)
    }
}

impl From<ProtoConversionError> for CliError {
    fn from(e: ProtoConversionError) -> Self {
        CliError::ProtoConversion(e)
    }
}

impl From<TransactProtoConversionError> for CliError {
    fn from(e: TransactProtoConversionError) -> Self {
        CliError::TransactProtoConversion(e)
    }
}

// used to convert BuildErrors into a CliError.
macro_rules! impl_builder_errors {
    ($($x:ty),*) => {
        $(
            impl From<$x> for CliError {
                fn from(e: $x) -> Self {
                    CliError::ProtocolBuild(Box::new(e))
                }
            }
        )*
    };
}

impl_builder_errors!(
    ActionBuildError,
    BatchBuildError,
    TransactionBuildError,
    SabrePayloadBuildError
);
