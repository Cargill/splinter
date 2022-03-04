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

use serde::Deserialize;
use splinter::error::InternalError;
use splinter::service::MessageConverter;

use super::message::EchoMessage;

#[derive(Serialize, Deserialize)]
pub enum EchoByteMessage {
    Request {
        message: String,
        correlation_id: u64,
    },
    Response {
        message: String,
        correlation_id: u64,
    },
}

#[derive(Clone)]
pub struct EchoMessageByteConverter {}

impl MessageConverter<EchoMessage, Vec<u8>> for EchoMessageByteConverter {
    fn to_left(&self, right: Vec<u8>) -> Result<EchoMessage, InternalError> {
        let msg: EchoByteMessage = serde_json::from_slice(&right)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        Ok(msg.into())
    }

    fn to_right(&self, left: EchoMessage) -> Result<Vec<u8>, InternalError> {
        serde_json::to_vec(&EchoByteMessage::from(left))
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

impl From<EchoByteMessage> for EchoMessage {
    fn from(msg: EchoByteMessage) -> Self {
        match msg {
            EchoByteMessage::Request {
                message,
                correlation_id,
            } => EchoMessage::Request {
                message,
                correlation_id,
            },
            EchoByteMessage::Response {
                message,
                correlation_id,
            } => EchoMessage::Response {
                message,
                correlation_id,
            },
        }
    }
}

impl From<EchoMessage> for EchoByteMessage {
    fn from(msg: EchoMessage) -> Self {
        match msg {
            EchoMessage::Request {
                message,
                correlation_id,
            } => EchoByteMessage::Request {
                message,
                correlation_id,
            },
            EchoMessage::Response {
                message,
                correlation_id,
            } => EchoByteMessage::Response {
                message,
                correlation_id,
            },
        }
    }
}
