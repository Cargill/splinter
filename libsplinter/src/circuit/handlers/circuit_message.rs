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
use crate::network::dispatch::{
    DispatchError, DispatchMessageSender, Handler, MessageContext, PeerId,
};
use crate::network::sender::NetworkMessageSender;
use crate::protos::circuit::{CircuitMessage, CircuitMessageType};
use crate::protos::network::NetworkMessageType;

// Implements a handler that pass messages to another dispatcher loop
pub struct CircuitMessageHandler {
    sender: DispatchMessageSender<CircuitMessageType>,
}

impl Handler for CircuitMessageHandler {
    type Source = PeerId;
    type MessageType = NetworkMessageType;
    type Message = CircuitMessage;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::CIRCUIT
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _: &NetworkMessageSender,
    ) -> Result<(), DispatchError> {
        debug!(
            "Handle CircuitMessage {:?} from {} [{} byte{}]",
            msg.get_message_type(),
            context.source_peer_id(),
            msg.get_payload().len(),
            if msg.get_payload().len() == 1 {
                ""
            } else {
                "s"
            }
        );

        self.sender
            .send(
                msg.get_message_type(),
                msg.get_payload().to_vec(),
                context.source_id().clone(),
            )
            .map_err(|_| {
                DispatchError::NetworkSendError((context.source_peer_id().to_string(), msg.payload))
            })?;
        Ok(())
    }
}

impl CircuitMessageHandler {
    pub fn new(sender: DispatchMessageSender<CircuitMessageType>) -> Self {
        CircuitMessageHandler { sender }
    }
}

#[cfg(test)]
mod tests {

    use std::sync::{Arc, RwLock};
    use std::{thread, time};

    use super::*;
    use crate::mesh::Mesh;
    use crate::network::dispatch::{DispatchLoopBuilder, Dispatcher};
    use crate::network::sender;
    use crate::network::Network;
    use crate::protos::circuit::ServiceConnectRequest;
    use crate::protos::network::NetworkMessageType;

    use protobuf::Message;

    #[test]
    // Test that circuit message is sent to the circuit dispatch sender
    fn test_circuit_message_handler() {
        // Set up dispatcher and mock sender
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut network_dispatcher = Dispatcher::new(network_sender.clone());

        let mut circuit_dispatcher = Dispatcher::new(network_sender);
        let handler = ServiceConnectedTestHandler::default();
        let echos = handler.echos.clone();
        circuit_dispatcher.set_handler(Box::new(handler));

        let circuit_dispatcher_loop = DispatchLoopBuilder::new()
            .with_dispatcher(circuit_dispatcher)
            .build()
            .unwrap();
        let circuit_dispatcher_message_sender = circuit_dispatcher_loop.new_dispatcher_sender();

        let handler = CircuitMessageHandler::new(circuit_dispatcher_message_sender);
        network_dispatcher.set_handler(Box::new(handler));

        // Create ServiceConnectRequest
        let mut service_request = ServiceConnectRequest::new();
        service_request.set_service_id("TEST_SERVICE".to_string());
        let service_msg_bytes = service_request.write_to_bytes().unwrap();
        // Create a CircuitMessage
        let mut circuit_msg = CircuitMessage::new();
        circuit_msg.set_message_type(CircuitMessageType::SERVICE_CONNECT_REQUEST);
        circuit_msg.set_payload(service_msg_bytes);
        let circuit_bytes = circuit_msg.write_to_bytes().unwrap();

        // Dispatch network message
        network_dispatcher
            .dispatch(
                "PEER".into(),
                &NetworkMessageType::CIRCUIT,
                circuit_bytes.clone(),
            )
            .unwrap();

        let mut count = 0;
        let ten_millis = time::Duration::from_millis(10);
        // give the dispatcher a chance to pass the message to the circuit dispatcher
        while echos.read().unwrap().is_empty() && count < 10 {
            thread::sleep(ten_millis);
            count += 1;
        }

        assert_eq!(
            vec!["TEST_SERVICE".to_string()],
            echos.read().unwrap().clone()
        );
    }

    #[derive(Default)]
    struct ServiceConnectedTestHandler {
        echos: Arc<RwLock<Vec<String>>>,
    }

    impl Handler for ServiceConnectedTestHandler {
        type Source = PeerId;
        type MessageType = CircuitMessageType;
        type Message = ServiceConnectRequest;

        fn match_type(&self) -> Self::MessageType {
            CircuitMessageType::SERVICE_CONNECT_REQUEST
        }

        fn handle(
            &self,
            message: Self::Message,
            _message_context: &MessageContext<Self::Source, Self::MessageType>,
            _: &NetworkMessageSender,
        ) -> Result<(), DispatchError> {
            self.echos
                .write()
                .unwrap()
                .push(message.get_service_id().to_string());
            Ok(())
        }
    }
}
