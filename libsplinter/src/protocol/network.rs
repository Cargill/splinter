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

use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;

use super::authorization::AuthorizationMessage;

/// The network message envelope
#[derive(Debug)]
pub enum NetworkMessage {
    NetworkEcho(NetworkEcho),
    NetworkHeartbeat(NetworkHeartbeat),
    Circuit(Vec<u8>),
    Authorization(AuthorizationMessage),
}

/// This message is used for debugging
#[derive(Debug)]
pub struct NetworkEcho {
    pub payload: Vec<u8>,
    pub recipient: String,
    pub time_to_live: i32,
}

/// This message is used to keep connections alive
#[derive(Debug)]
pub struct NetworkHeartbeat;

impl FromProto<network::NetworkEcho> for NetworkEcho {
    fn from_proto(mut source: network::NetworkEcho) -> Result<Self, ProtoConversionError> {
        Ok(Self {
            payload: source.take_payload(),
            recipient: source.take_recipient(),
            time_to_live: source.get_time_to_live(),
        })
    }
}

impl FromNative<NetworkEcho> for network::NetworkEcho {
    fn from_native(source: NetworkEcho) -> Result<Self, ProtoConversionError> {
        let mut proto_request = network::NetworkEcho::new();
        proto_request.set_payload(source.payload);
        proto_request.set_recipient(source.recipient);
        proto_request.set_time_to_live(source.time_to_live);

        Ok(proto_request)
    }
}

impl FromProto<network::NetworkHeartbeat> for NetworkHeartbeat {
    fn from_proto(_: network::NetworkHeartbeat) -> Result<Self, ProtoConversionError> {
        Ok(NetworkHeartbeat)
    }
}

impl FromNative<NetworkHeartbeat> for network::NetworkHeartbeat {
    fn from_native(_: NetworkHeartbeat) -> Result<Self, ProtoConversionError> {
        Ok(network::NetworkHeartbeat::new())
    }
}

impl FromProto<network::NetworkMessage> for NetworkMessage {
    fn from_proto(mut source: network::NetworkMessage) -> Result<Self, ProtoConversionError> {
        use network::NetworkMessageType::*;
        match source.message_type {
            NETWORK_ECHO => Ok(NetworkMessage::NetworkEcho(FromBytes::<
                network::NetworkEcho,
            >::from_bytes(
                source.get_payload()
            )?)),
            NETWORK_HEARTBEAT => Ok(NetworkMessage::NetworkHeartbeat(FromBytes::<
                network::NetworkHeartbeat,
            >::from_bytes(
                source.get_payload()
            )?)),
            CIRCUIT => Ok(NetworkMessage::Circuit(source.take_payload())),
            AUTHORIZATION => Ok(NetworkMessage::Authorization(
                AuthorizationMessage::from_bytes(source.get_payload())?,
            )),
            UNSET_NETWORK_MESSAGE_TYPE => Err(ProtoConversionError::InvalidTypeError(
                "no message type was set".into(),
            )),
        }
    }
}

impl FromNative<NetworkMessage> for network::NetworkMessage {
    fn from_native(source: NetworkMessage) -> Result<Self, ProtoConversionError> {
        use network::NetworkMessageType::*;

        let mut message = network::NetworkMessage::new();
        match source {
            NetworkMessage::NetworkEcho(payload) => {
                message.set_message_type(NETWORK_ECHO);
                message.set_payload(IntoBytes::<network::NetworkEcho>::into_bytes(payload)?);
            }
            NetworkMessage::NetworkHeartbeat(payload) => {
                message.set_message_type(NETWORK_HEARTBEAT);
                message.set_payload(IntoBytes::<network::NetworkHeartbeat>::into_bytes(payload)?);
            }
            NetworkMessage::Circuit(payload) => {
                message.set_message_type(CIRCUIT);
                message.set_payload(payload);
            }
            NetworkMessage::Authorization(payload) => {
                message.set_message_type(AUTHORIZATION);
                message.set_payload(
                    IntoBytes::<authorization::AuthorizationMessage>::into_bytes(payload)?,
                );
            }
        }
        Ok(message)
    }
}

impl From<AuthorizationMessage> for NetworkMessage {
    fn from(auth_message: AuthorizationMessage) -> Self {
        NetworkMessage::Authorization(auth_message)
    }
}
