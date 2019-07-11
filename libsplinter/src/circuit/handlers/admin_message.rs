// Copyright 2019 Cargill Incorporated
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

use std::sync::{Arc, RwLock};

use crate::channel::Sender;
use crate::circuit::handlers::create_message;
use crate::circuit::{ServiceId, SplinterState};
use crate::network::dispatch::{DispatchError, Handler, MessageContext};
use crate::network::sender::SendRequest;
use crate::protos::circuit::{
    AdminDirectMessage, CircuitError, CircuitError_Error, CircuitMessageType,
};
use crate::rwlock_read_unwrap;
use protobuf::Message;

// Implements a handler that handles AdminDirectMessage
pub struct AdminDirectMessageHandler {
    node_id: String,
    state: Arc<RwLock<SplinterState>>,
}

impl Handler<CircuitMessageType, AdminDirectMessage> for AdminDirectMessageHandler {
    fn handle(
        &self,
        msg: AdminDirectMessage,
        context: &MessageContext<CircuitMessageType>,
        sender: &dyn Sender<SendRequest>,
    ) -> Result<(), DispatchError> {
        debug!("Handle Admin Direct Message {:?}", msg);
        let circuit_name = msg.get_circuit();
        let msg_sender = msg.get_sender();
        let recipient = msg.get_recipient();
        let recipient_id = ServiceId::new(circuit_name.to_string(), recipient.to_string());
        let sender_id = ServiceId::new(circuit_name.to_string(), msg_sender.to_string());

        // Get read lock on state
        let state = rwlock_read_unwrap!(self.state);

        // msg bytes will either be message bytes of a direct message or an error message
        // the msg_recipient is either the service/node id to send the message to or is the
        // peer_id to send back the error message
        let (msg_bytes, msg_recipient) = {
            if let Some(circuit) = state.circuit(circuit_name) {
                // Check if the message sender is allowed on the circuit
                // if the sender is not allowed on the circuit
                if !circuit.roster().contains(&msg_sender) {
                    let msg_bytes = create_circuit_error_msg(
                        &msg,
                        CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER,
                        format!("Sender is not allowed in the Circuit: {}", msg_sender),
                    )?;

                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id())
                } else if state.service_directory().get(&sender_id).is_none() {
                    // Check if the message sender is registered on the circuit
                    // if the sender is not connected, send circuit error
                    let msg_bytes = create_circuit_error_msg(
                        &msg,
                        CircuitError_Error::ERROR_SENDER_NOT_IN_DIRECTORY,
                        format!("Sender is not in the service directory: {}", recipient),
                    )?;

                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id())
                } else if circuit.roster().contains(&recipient) {
                    // check if the recipient service is allowed on the circuit and registered
                    if let Some(service) = state.service_directory().get(&recipient_id) {
                        let node_id = service.node().id();
                        // If the service is on this node send message to the service, otherwise
                        // send the message to the node the service is connected to
                        if node_id != self.node_id {
                            let msg_bytes = context.message_bytes().to_vec();
                            let network_msg_bytes = create_message(
                                msg_bytes,
                                CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                            )?;
                            (network_msg_bytes, node_id)
                        } else {
                            let msg_bytes = context.message_bytes().to_vec();
                            let network_msg_bytes = create_message(
                                msg_bytes,
                                CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                            )?;
                            let peer_id = match service.peer_id() {
                                Some(peer_id) => peer_id,
                                None => {
                                    // This should never happen, as a peer id will always
                                    // be set on a service that is connected to the local node.
                                    warn!("No peer id for service:{} ", service.service_id());
                                    return Ok(());
                                }
                            };
                            (network_msg_bytes, &peer_id[..])
                        }
                    } else {
                        // if the recipient is not connected, send circuit error
                        let msg_bytes = create_circuit_error_msg(
                            &msg,
                            CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY,
                            format!("Recipient is not in the service directory: {}", recipient),
                        )?;

                        let network_msg_bytes =
                            create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                        (network_msg_bytes, context.source_peer_id())
                    }
                } else {
                    // if the recipient is not allowed on the circuit, send circuit error
                    let msg_bytes = create_circuit_error_msg(
                        &msg,
                        CircuitError_Error::ERROR_RECIPIENT_NOT_IN_CIRCUIT_ROSTER,
                        format!("Recipient is not allowed in the Circuit: {}", recipient),
                    )?;

                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id())
                }
            } else {
                // if the circuit does not exist, send circuit error
                let msg_bytes = create_circuit_error_msg(
                    &msg,
                    CircuitError_Error::ERROR_CIRCUIT_DOES_NOT_EXIST,
                    format!("Circuit does not exist: {}", circuit_name),
                )?;

                let network_msg_bytes =
                    create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                (network_msg_bytes, context.source_peer_id())
            }
        };

        // either forward the direct message or send back an error message.
        let send_request = SendRequest::new(msg_recipient.to_string(), msg_bytes);
        sender.send(send_request)?;
        Ok(())
    }
}

impl AdminDirectMessageHandler {
    pub fn new(node_id: String, state: Arc<RwLock<SplinterState>>) -> Self {
        Self { node_id, state }
    }
}

fn create_circuit_error_msg(
    msg: &AdminDirectMessage,
    error_type: CircuitError_Error,
    error_msg: String,
) -> Result<Vec<u8>, DispatchError> {
    let mut error_message = CircuitError::new();
    error_message.set_correlation_id(msg.get_correlation_id().into());
    error_message.set_service_id(msg.get_sender().into());
    error_message.set_circuit_name(msg.get_circuit().into());
    error_message.set_error(error_type);
    error_message.set_error_message(error_msg);

    error_message.write_to_bytes().map_err(DispatchError::from)
}
