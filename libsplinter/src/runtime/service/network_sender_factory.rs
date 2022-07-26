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

use std::sync::Arc;

use protobuf::Message;

use crate::circuit::routing::{RoutingTableReader, ServiceId as RoutingServiceId};
use crate::error::InternalError;
use crate::network::dispatch::{MessageSender as NetworkDispatchMessageSender, PeerId};
use crate::peer::PeerTokenPair;
use crate::protos::circuit::{CircuitDirectMessage, CircuitMessage, CircuitMessageType};
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::service::{FullyQualifiedServiceId, MessageSender, MessageSenderFactory, ServiceId};

#[derive(Clone)]
pub struct NetworkMessageSenderFactory<S>
where
    S: NetworkDispatchMessageSender<PeerId> + Clone + 'static,
{
    node_id: Arc<str>,
    network_dispatch_message_sender: S,
    routing_table_reader: Box<dyn RoutingTableReader>,
}

impl<S> NetworkMessageSenderFactory<S>
where
    S: NetworkDispatchMessageSender<PeerId> + Clone + 'static,
{
    pub fn new(
        node_id: &str,
        network_dispatch_message_sender: S,
        routing_table_reader: Box<dyn RoutingTableReader>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            network_dispatch_message_sender,
            routing_table_reader,
        }
    }
}

impl<S> MessageSenderFactory<Vec<u8>> for NetworkMessageSenderFactory<S>
where
    S: NetworkDispatchMessageSender<PeerId> + Clone + 'static,
{
    fn new_message_sender(
        &self,
        scope: &FullyQualifiedServiceId,
    ) -> Result<Box<dyn MessageSender<Vec<u8>>>, InternalError> {
        Ok(Box::new(NetworkMessageSender {
            node_id: self.node_id.clone(),
            scope: scope.clone(),
            network_dispatch_message_sender: self.network_dispatch_message_sender.clone(),
            routing_table_reader: self.routing_table_reader.clone(),
        }))
    }

    fn clone_boxed(&self) -> Box<dyn MessageSenderFactory<Vec<u8>>> {
        Box::new(self.clone())
    }
}

pub struct NetworkMessageSender<S> {
    node_id: Arc<str>,
    scope: FullyQualifiedServiceId,
    network_dispatch_message_sender: S,
    routing_table_reader: Box<dyn RoutingTableReader>,
}

impl<S> MessageSender<Vec<u8>> for NetworkMessageSender<S>
where
    S: NetworkDispatchMessageSender<PeerId>,
{
    fn send(&self, to_service: &ServiceId, message: Vec<u8>) -> Result<(), InternalError> {
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit(self.scope.circuit_id().to_string());
        direct_message.set_sender(self.scope.service_id().to_string());

        direct_message.set_recipient(to_service.as_str().to_string());
        direct_message.set_payload(message);

        let bytes = direct_message
            .write_to_bytes()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let circuit = self
            .routing_table_reader
            .get_circuit(self.scope.circuit_id().as_str())
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!(
                    "Circuit {} is not routable",
                    self.scope.circuit_id(),
                ))
            })?;

        let service = self
            .routing_table_reader
            .get_service(&RoutingServiceId::new(
                self.scope.circuit_id().to_string(),
                to_service.to_string(),
            ))
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!(
                    "Service {}::{} is not routable",
                    self.scope.circuit_id(),
                    to_service
                ))
            })?;

        let remote_peer_id = self
            .routing_table_reader
            .get_node(service.node_id())
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!(
                    "Service {}::{} is on an unknown peer",
                    self.scope.circuit_id(),
                    to_service
                ))
            })?
            .get_peer_auth_token(circuit.authorization_type())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let local_peer_id = self
            .routing_table_reader
            .get_node(&self.node_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!(
                    "Unable to lookup local node with node id {}",
                    self.node_id,
                ))
            })?
            .get_peer_auth_token(circuit.authorization_type())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let target_peer_id: PeerId = PeerTokenPair::new(remote_peer_id, local_peer_id).into();

        let msg = create_message(bytes, CircuitMessageType::CIRCUIT_DIRECT_MESSAGE)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.network_dispatch_message_sender
            .send(target_peer_id, msg)
            .map_err(|(peer, _msg)| {
                InternalError::with_message(format!("Unable to send message to {}", peer))
            })?;

        Ok(())
    }
}

/// Helper function for creating a NetworkMessage with a Circuit message type
///
/// # Arguments
///
/// * `payload` - The payload in bytes that should be set in the Circuit message get_payload
/// * `circuit_message_type` - The message type that should be set in the Circuit message
fn create_message(
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
