// Copyright 2018-2022 Cargill Incorporated
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

use tokio::io;

use super::WebSocketError;

#[derive(Debug)]
pub enum ReactorError {
    WsStartError(String),
    ListenError(WebSocketError),
    RequestSendError(String),
    ReactorShutdownError(String),
    ShutdownHandleErrors(Vec<WebSocketError>),
    IoError(io::Error),
}

impl Error for ReactorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReactorError::ListenError(err) => Some(err),
            ReactorError::WsStartError(_) => None,
            ReactorError::RequestSendError(_) => None,
            ReactorError::ReactorShutdownError(_) => None,
            ReactorError::ShutdownHandleErrors(_) => None,
            ReactorError::IoError(err) => Some(err),
        }
    }
}

impl fmt::Display for ReactorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReactorError::ListenError(err) => write!(f, "{}", err),
            ReactorError::WsStartError(err) => write!(f, "{}", err),
            ReactorError::RequestSendError(err) => write!(f, "{}", err),
            ReactorError::ReactorShutdownError(err) => write!(f, "{}", err),
            ReactorError::ShutdownHandleErrors(err) => {
                let err_message = err
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "Websockets did not shut down correctly: {}", err_message)
            }
            ReactorError::IoError(err) => write!(f, "IO Error: {}", err),
        }
    }
}

impl From<io::Error> for ReactorError {
    fn from(err: io::Error) -> Self {
        ReactorError::IoError(err)
    }
}

impl From<WebSocketError> for ReactorError {
    fn from(err: WebSocketError) -> Self {
        ReactorError::ListenError(err)
    }
}
