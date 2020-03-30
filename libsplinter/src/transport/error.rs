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

#[derive(Debug)]
pub enum AcceptError {
    IoError(io::Error),
    ProtocolError(String),
}

impl Error for AcceptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AcceptError::IoError(err) => Some(err),
            AcceptError::ProtocolError(_) => None,
        }
    }
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AcceptError::IoError(err) => write!(f, "io error occurred: {}", err),
            AcceptError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
        }
    }
}

impl From<io::Error> for AcceptError {
    fn from(io_error: io::Error) -> Self {
        AcceptError::IoError(io_error)
    }
}

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

impl From<io::Error> for ConnectError {
    fn from(io_error: io::Error) -> Self {
        ConnectError::IoError(io_error)
    }
}

#[derive(Debug)]
pub enum DisconnectError {
    IoError(io::Error),
    ProtocolError(String),
}

impl Error for DisconnectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DisconnectError::IoError(err) => Some(err),
            DisconnectError::ProtocolError(_) => None,
        }
    }
}

impl std::fmt::Display for DisconnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DisconnectError::IoError(err) => write!(f, "io error occurred: {}", err),
            DisconnectError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
        }
    }
}

impl From<io::Error> for DisconnectError {
    fn from(io_error: io::Error) -> Self {
        DisconnectError::IoError(io_error)
    }
}

#[derive(Debug)]
pub enum ListenError {
    IoError(io::Error),
    ProtocolError(String),
}

impl Error for ListenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ListenError::IoError(err) => Some(err),
            ListenError::ProtocolError(_) => None,
        }
    }
}

impl std::fmt::Display for ListenError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ListenError::IoError(err) => write!(f, "io error occurred: {}", err),
            ListenError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
        }
    }
}

impl From<io::Error> for ListenError {
    fn from(io_error: io::Error) -> Self {
        ListenError::IoError(io_error)
    }
}

#[derive(Debug)]
pub enum RecvError {
    IoError(io::Error),
    ProtocolError(String),
    WouldBlock,
    Disconnected,
}

impl Error for RecvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RecvError::IoError(err) => Some(err),
            RecvError::ProtocolError(_) => None,
            RecvError::WouldBlock => None,
            RecvError::Disconnected => None,
        }
    }
}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RecvError::IoError(err) => write!(f, "io error occurred: {}", err),
            RecvError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
            RecvError::WouldBlock => write!(f, "would block"),
            RecvError::Disconnected => write!(f, "disconnected"),
        }
    }
}

impl From<io::Error> for RecvError {
    fn from(io_error: io::Error) -> Self {
        match io_error.kind() {
            io::ErrorKind::UnexpectedEof => RecvError::Disconnected,
            io::ErrorKind::WouldBlock => RecvError::WouldBlock,
            _ => RecvError::IoError(io_error),
        }
    }
}

#[derive(Debug)]
pub enum SendError {
    IoError(io::Error),
    ProtocolError(String),
    WouldBlock,
    Disconnected,
}

impl Error for SendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendError::IoError(err) => Some(err),
            SendError::ProtocolError(_) => None,
            SendError::WouldBlock => None,
            SendError::Disconnected => None,
        }
    }
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SendError::IoError(err) => write!(f, "io error occurred: {}", err),
            SendError::ProtocolError(err) => write!(f, "protocol error occurred: {}", err),
            SendError::WouldBlock => write!(f, "would block"),
            SendError::Disconnected => write!(f, "disconnected"),
        }
    }
}

impl From<io::Error> for SendError {
    fn from(io_error: io::Error) -> Self {
        match io_error.kind() {
            io::ErrorKind::UnexpectedEof => SendError::Disconnected,
            io::ErrorKind::WouldBlock => SendError::WouldBlock,
            _ => SendError::IoError(io_error),
        }
    }
}
