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

//! Protocol structs for splinter component messages
//!
//! These structs are used to operate on the messages that are transmitted between components and a
//! splinter node.

use protobuf::Message;

use crate::protocol::service::ServiceMessage;
use crate::protos::component;
use crate::protos::prelude::*;
use crate::protos::service;

/// The component message envelope.  All message sent to local components will be wrapped in one of
/// these.
pub enum ComponentMessage {
    /// A message to/from service components.
    Service(ServiceMessage),
    /// A keep-alive message.
    Heartbeat,
}

impl FromProto<component::ComponentMessage> for ComponentMessage {
    fn from_proto(msg: component::ComponentMessage) -> Result<Self, ProtoConversionError> {
        use component::ComponentMessageType::*;
        match msg.get_message_type() {
            SERVICE => Ok(ComponentMessage::Service(FromBytes::<
                service::ServiceMessage,
            >::from_bytes(
                msg.get_payload()
            )?)),
            COMPONENT_HEARTBEAT => Ok(ComponentMessage::Heartbeat),
            UNSET_COMPONENT_MESSAGE_TYPE => Err(ProtoConversionError::InvalidTypeError(
                "message type not set".into(),
            )),
        }
    }
}

impl FromNative<ComponentMessage> for component::ComponentMessage {
    fn from_native(msg: ComponentMessage) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = component::ComponentMessage::new();
        use component::ComponentMessageType::*;
        match msg {
            ComponentMessage::Service(service_msg) => {
                proto_msg.set_message_type(SERVICE);
                proto_msg.set_payload(IntoBytes::<service::ServiceMessage>::into_bytes(
                    service_msg,
                )?);
            }
            ComponentMessage::Heartbeat => {
                proto_msg.set_message_type(COMPONENT_HEARTBEAT);
                proto_msg.set_payload(
                    component::ComponentHeartbeat::new()
                        .write_to_bytes()
                        .map_err(|err| ProtoConversionError::SerializationError(err.to_string()))?,
                );
            }
        }

        Ok(proto_msg)
    }
}
