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

//! Dispatch handlers for service component messages.

use crate::circuit::service::ServiceId;
use crate::network::dispatch::{
    ConnectionId, DispatchError, DispatchMessageSender, Handler, MessageContext, MessageSender,
};
use crate::protos::component;
use crate::protos::service;

/// Dispatch handler for the service message envelope.
pub struct ServiceMessageHandler {
    sender: DispatchMessageSender<service::ServiceMessageType, ConnectionId>,
}

impl ServiceMessageHandler {
    /// Construct a new `ServiceMessageHandler` with a `DispatchMessageSender` for the contents of
    /// the envelope.
    pub fn new(sender: DispatchMessageSender<service::ServiceMessageType, ConnectionId>) -> Self {
        Self { sender }
    }
}

impl Handler for ServiceMessageHandler {
    type Source = ConnectionId;
    type MessageType = component::ComponentMessageType;
    type Message = service::ServiceMessage;

    fn match_type(&self) -> Self::MessageType {
        component::ComponentMessageType::SERVICE
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let msg_type = msg.get_message_type();
        let payload = msg.take_payload();
        let circuit = msg.take_circuit();
        let service_id = msg.take_service_id();
        self.sender
            .send_with_parent_context(
                msg_type,
                payload,
                context.source_id().clone(),
                Box::new(ServiceId::new(circuit, service_id)),
            )
            .map_err(|_| {
                DispatchError::NetworkSendError((
                    context.source_connection_id().to_string(),
                    msg.payload,
                ))
            })
    }
}
