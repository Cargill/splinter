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

//! trait implementations to support messages that are sent from service components.

use protobuf::Message;

use crate::circuit::service::{ServiceId, SplinterNode};
use crate::circuit::SplinterState;
use crate::peer::interconnect::NetworkMessageSender;
use crate::protocol::service::ServiceProcessorMessage;
use crate::protos::circuit::{
    AdminDirectMessage, CircuitDirectMessage, CircuitMessage, CircuitMessageType,
};
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::service::network::{ForwardResult, ServiceForwardingError, ServiceMessageForwarder};

const ADMIN_SERVICE_ID_PREFIX: &str = "admin::";

///
pub struct NetworkMessageForwarder {
    splinter_node: SplinterNode,
    network_sender: NetworkMessageSender,
    state: SplinterState,
}

impl ServiceMessageForwarder for NetworkMessageForwarder {
    fn forward(
        &self,
        service_id: &ServiceId,
        service_msg: ServiceProcessorMessage,
    ) -> Result<ForwardResult, ServiceForwardingError> {
        if service_id.circuit() == "admim" {
            self.forward_admin_message(service_id, service_msg)
        } else {
            self.forward_general_circuit_message(service_id, service_msg)
        }
    }
}

impl NetworkMessageForwarder {
    fn forward_admin_message(
        &self,
        service_id: &ServiceId,
        service_msg: ServiceProcessorMessage,
    ) -> Result<ForwardResult, ServiceForwardingError> {
        if !is_admin_service_id(service_id.service_id()) {
            return Err(ServiceForwardingError::SenderNotInCircuit);
        }

        if !is_admin_service_id(&service_msg.recipient) {
            return Err(ServiceForwardingError::RecipientNotInCircuit);
        }

        let target_node_id = (&service_msg.recipient[ADMIN_SERVICE_ID_PREFIX.len()..]).to_string();
        let mut admin_direct_message = AdminDirectMessage::new();
        admin_direct_message.set_circuit("admin".into());
        admin_direct_message.set_sender(service_id.service_id().to_string());
        admin_direct_message.set_recipient(service_msg.recipient);
        admin_direct_message.set_payload(service_msg.payload);

        if let Some(correlation_id) = service_msg.correlation_id {
            admin_direct_message.set_correlation_id(correlation_id);
        }

        let bytes = admin_direct_message.write_to_bytes().map_err(|err| {
            ServiceForwardingError::InternalError {
                context: "unable to serialize admin direct message envelope".into(),
                source: Some(Box::new(err)),
            }
        })?;

        let msg =
            create_message(bytes, CircuitMessageType::ADMIN_DIRECT_MESSAGE).map_err(|err| {
                ServiceForwardingError::InternalError {
                    context: "unable to create circuit/network envelope".into(),
                    source: Some(Box::new(err)),
                }
            })?;

        self.network_sender
            .send(target_node_id, msg)
            .map(|_| ForwardResult::Sent)
            .map_err(|(target, _)| ServiceForwardingError::InternalError {
                context: format!("unable to send admin direct message to {}", target),
                source: None,
            })
    }

    fn forward_general_circuit_message(
        &self,
        service_id: &ServiceId,
        service_msg: ServiceProcessorMessage,
    ) -> Result<ForwardResult, ServiceForwardingError> {
        let circuit = self
            .state
            .circuit(service_id.circuit())
            .map_err(|err| ServiceForwardingError::InternalError {
                context: "unable to look up circuit".into(),
                source: Some(Box::new(err)),
            })?
            .ok_or_else(|| ServiceForwardingError::CircuitDoesNotExist)?;

        if !circuit.roster().contains(service_id.service_id()) {
            return Err(ServiceForwardingError::SenderNotInCircuit);
        }

        if !circuit.roster().contains(&service_msg.recipient) {
            return Err(ServiceForwardingError::RecipientNotInCircuit);
        }

        let recipient_id = ServiceId::new(
            service_id.circuit().to_string(),
            service_msg.recipient.clone(),
        );

        // validate that the services are in the directory
        self.state
            .get_service(service_id)
            .map_err(|err| ServiceForwardingError::InternalError {
                context: format!("unable to look up service {}", service_id),
                source: Some(Box::new(err)),
            })?
            .ok_or_else(|| ServiceForwardingError::SenderNotRegistered)?;

        let recipient_service = self
            .state
            .get_service(&recipient_id)
            .map_err(|err| ServiceForwardingError::InternalError {
                context: format!("unable to look up service {}", &recipient_id),
                source: Some(Box::new(err)),
            })?
            .ok_or_else(|| ServiceForwardingError::RecipientNotRegistered)?;

        if self.splinter_node.id() != recipient_service.node().id() {
            // Construct a circuit direct message, and send it to the service peer:
            let (circuit_name, recipient_id) = recipient_id.into_parts();
            let mut direct_msg = CircuitDirectMessage::new();
            direct_msg.set_circuit(circuit_name);
            direct_msg.set_sender(service_id.service_id().to_string());
            direct_msg.set_recipient(recipient_id);
            direct_msg.set_payload(service_msg.payload);
            if let Some(correlation_id) = service_msg.correlation_id {
                direct_msg.set_correlation_id(correlation_id);
            }

            let bytes = direct_msg.write_to_bytes().map_err(|err| {
                ServiceForwardingError::InternalError {
                    context: "unable to serialize circuit direct message envelope".into(),
                    source: Some(Box::new(err)),
                }
            })?;

            let msg = create_message(bytes, CircuitMessageType::CIRCUIT_DIRECT_MESSAGE).map_err(
                |err| ServiceForwardingError::InternalError {
                    context: "unable to create circuit/network envelope".into(),
                    source: Some(Box::new(err)),
                },
            )?;

            self.network_sender
                .send(recipient_service.node().id().into(), msg)
                .map(|_| ForwardResult::Sent)
                .map_err(|_| ServiceForwardingError::InternalError {
                    context: format!(
                        "unable to send admin direct message to {}",
                        recipient_service.node().id()
                    ),
                    source: None,
                })
        } else if let Some(component_id) = recipient_service.peer_id() {
            Ok(ForwardResult::LocalReReroute(
                component_id.into(),
                service_msg,
            ))
        } else {
            Err(ServiceForwardingError::InternalError {
                context: format!(
                    "Service {} was registered without a component id",
                    recipient_service.service_id()
                ),
                source: None,
            })
        }
    }
}

/// Check if the service id is an admin service.
fn is_admin_service_id(service_id: &str) -> bool {
    service_id.starts_with(ADMIN_SERVICE_ID_PREFIX)
}

/// Helper function for creating a NetworkMessge with a Circuit message type
///
/// # Arguments
///
/// * `payload` - The payload in bytes that should be set in the Circuit message get_payload
/// * `circuit_message_type` - The message type that should be set in teh Circuit message
pub fn create_message(
    payload: Vec<u8>,
    circuit_message_type: CircuitMessageType,
) -> Result<Vec<u8>, protobuf::error::ProtobufError> {
    let mut circuit_msg = CircuitMessage::new();
    circuit_msg.set_message_type(circuit_message_type);
    circuit_msg.set_payload(payload);
    let circuit_bytes = circuit_msg.write_to_bytes()?;

    let mut network_msg = NetworkMessage::new();
    network_msg.set_message_type(NetworkMessageType::CIRCUIT);
    network_msg.set_payload(circuit_bytes);
    network_msg.write_to_bytes()
}
