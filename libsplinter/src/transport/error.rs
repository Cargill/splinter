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
use std::io;

macro_rules! impl_from_io_error {
    ($err:ident) => {
        impl From<io::Error> for $err {
            fn from(io_error: io::Error) -> Self {
                $err::IoError(io_error)
            }
        }
    };
}

macro_rules! impl_from_io_error_ext {
    ($err:ident) => {
        impl From<io::Error> for $err {
            fn from(io_error: io::Error) -> Self {
                match io_error.kind() {
                    io::ErrorKind::UnexpectedEof => $err::Disconnected,
                    io::ErrorKind::WouldBlock => $err::WouldBlock,
                    _ => $err::IoError(io_error),
                }
            }
        }
    };
}

#[derive(Debug)]
pub enum AcceptError {
    IoError(io::Error),
    ProtocolError(String),
}

impl_from_io_error!(AcceptError);

#[derive(Debug)]
pub enum ConnectError {
    IoError(io::Error),
    ParseError(String),
    ProtocolError(String),
}

impl Error for ConnectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConnectError::IoError(err) => Some(err),
            ConnectError::ParseError(_) => None,
            ConnectError::ProtocolError(_) => None,
        }
    }
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConnectError::IoError(err) => write!(f, "io error occurred: {}", err),
            ConnectError::ParseError(err) => write!(f, "error while parsing: {}", err),
            ConnectError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
        }
    }
}

impl_from_io_error!(ConnectError);

#[derive(Debug)]
pub enum DisconnectError {
    IoError(io::Error),
    ProtocolError(String),
}

impl_from_io_error!(DisconnectError);

#[derive(Debug)]
pub enum ListenError {
    IoError(io::Error),
    ProtocolError(String),
}

impl_from_io_error!(ListenError);

#[derive(Debug)]
pub enum PollError {}

#[derive(Debug)]
pub enum RecvError {
    IoError(io::Error),
    ProtocolError(String),
    WouldBlock,
    Disconnected,
}

impl_from_io_error_ext!(RecvError);

#[derive(Debug)]
pub enum SendError {
    IoError(io::Error),
    ProtocolError(String),
    WouldBlock,
    Disconnected,
}

impl_from_io_error_ext!(SendError);

#[derive(Debug)]
pub enum StatusError {}
