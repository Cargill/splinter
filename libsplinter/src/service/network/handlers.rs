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

/// A mapping of service instances and the component responsible for it.  This can be used to add
/// or remove service connection information.
pub trait ServiceInstances {
    /// Add a service instance.
    ///
    /// This method should create an association of the service with the given component id.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceAddInstanceError` if the service cannot be added.
    fn add_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceAddInstanceError>;

    /// Remove a service instance.
    ///
    /// This method should remove the association of the service with the given component id.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceRemoveInstanceError` if the service cannot be removed.
    fn remove_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceRemoveInstanceError>;
}

/// Errors that may occur on registration.
#[derive(Debug)]
pub enum ServiceAddInstanceError {
    /// The service is not allowed to register for the given circuit on this node.
    NotAllowed,
    /// The service is already registered.
    AlreadyRegistered,
    /// The service does not belong to the specified circuit.
    NotInCircuit,
    /// The specified circuit does not exist.
    CircuitDoesNotExist,
    /// An internal error has occurred while processing the service registration.
    InternalError {
        context: String,
        source: Option<Box<dyn std::error::Error + Send>>,
    },
}

impl std::error::Error for ServiceAddInstanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServiceAddInstanceError::InternalError {
                source: Some(ref err),
                ..
            } => Some(&**err),
            _ => None,
        }
    }
}

impl std::fmt::Display for ServiceAddInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ServiceAddInstanceError::NotAllowed => f.write_str("service not allowed on this node"),
            ServiceAddInstanceError::AlreadyRegistered => f.write_str("service already registered"),
            ServiceAddInstanceError::NotInCircuit => f.write_str("service is not in the circuit"),
            ServiceAddInstanceError::CircuitDoesNotExist => f.write_str("circuit does not exist"),
            ServiceAddInstanceError::InternalError {
                context,
                source: Some(ref err),
            } => write!(f, "{}: {}", context, err),
            ServiceAddInstanceError::InternalError {
                context,
                source: None,
            } => f.write_str(&context),
        }
    }
}

/// Errors that may occur on deregistration.
#[derive(Debug)]
pub enum ServiceRemoveInstanceError {
    /// The service is not currently registered with this node.
    NotRegistered,
    /// The service does not belong to the specified circuit.
    NotInCircuit,
    /// The specified circuit does not exist.
    CircuitDoesNotExist,
    /// An internal error has occurred while processing the service deregistration.
    InternalError {
        context: String,
        source: Option<Box<dyn std::error::Error + Send>>,
    },
}

impl std::error::Error for ServiceRemoveInstanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServiceRemoveInstanceError::InternalError {
                source: Some(ref err),
                ..
            } => Some(&**err),
            _ => None,
        }
    }
}

impl std::fmt::Display for ServiceRemoveInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ServiceRemoveInstanceError::NotRegistered => f.write_str("service is not registered"),
            ServiceRemoveInstanceError::NotInCircuit => {
                f.write_str("service is not in the circuit")
            }
            ServiceRemoveInstanceError::CircuitDoesNotExist => {
                f.write_str("circuit does not exist")
            }
            ServiceRemoveInstanceError::InternalError {
                context,
                source: Some(ref err),
            } => write!(f, "{}: {}", context, err),
            ServiceRemoveInstanceError::InternalError {
                context,
                source: None,
            } => f.write_str(&context),
        }
    }
}
