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
use crate::protocol::component::ComponentMessage;
use crate::protocol::service::{
    ConnectResponseStatus, DisconnectResponseStatus, ErrorKind, ServiceConnectResponse,
    ServiceDisconnectResponse, ServiceErrorMessage, ServiceMessage, ServiceMessagePayload,
    ServiceProcessorMessage,
};
use crate::protos::component;
use crate::protos::prelude::*;
use crate::protos::service;

use super::error::{ServiceAddInstanceError, ServiceForwardingError, ServiceRemoveInstanceError};
use super::{ForwardResult, ServiceInstances, ServiceMessageForwarder};

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

/// Dispatch handler for `ServiceConnectRequest` messages.
///
/// This handler processes an incoming `ServiceConnectRequest` and sends a reply with the
/// appropriate status.
pub struct ServiceConnectRequestHandler {
    service_instances: Box<dyn ServiceInstances + Send>,
}

impl ServiceConnectRequestHandler {
    /// Construct a new handler with a given service instances implementation.
    pub fn new(service_instances: Box<dyn ServiceInstances + Send>) -> Self {
        Self { service_instances }
    }
}

impl Handler for ServiceConnectRequestHandler {
    type Source = ConnectionId;
    type MessageType = service::ServiceMessageType;
    type Message = service::SMConnectRequest;

    fn match_type(&self) -> Self::MessageType {
        service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let service_id: &ServiceId = context.get_parent_context().ok_or_else(|| {
            DispatchError::HandleError(
                "Service Connect Request not provided with service ID from envelope.".into(),
            )
        })?;

        let status = match self
            .service_instances
            .add_service_instance(service_id.clone(), context.source_connection_id().into())
        {
            Ok(()) => ConnectResponseStatus::Ok,
            Err(ServiceAddInstanceError::NotAllowed) => ConnectResponseStatus::NotAnAllowedNode(
                format!("Service {} is not allowed on this node", service_id),
            ),
            Err(ServiceAddInstanceError::AlreadyRegistered) => {
                ConnectResponseStatus::ServiceAlreadyRegistered(format!(
                    "Service {} is already registered",
                    service_id
                ))
            }
            Err(ServiceAddInstanceError::NotInCircuit) => {
                ConnectResponseStatus::ServiceNotInCircuitRegistry(format!(
                    "Service {} is not allowed in circuit {}",
                    service_id.service_id(),
                    service_id.circuit()
                ))
            }
            Err(ServiceAddInstanceError::CircuitDoesNotExist) => {
                ConnectResponseStatus::CircuitDoesNotExist(format!(
                    "Circuit {} does not exist",
                    service_id.circuit()
                ))
            }
            Err(err @ ServiceAddInstanceError::InternalError { .. }) => {
                error!("Unable to register service {}: {}", service_id, err);
                ConnectResponseStatus::InternalError("An internal error has occurred".into())
            }
        };

        let response = ComponentMessage::Service(ServiceMessage {
            circuit: service_id.circuit().to_string(),
            service_id: service_id.service_id().to_string(),
            payload: ServiceMessagePayload::ConnectResponse(ServiceConnectResponse {
                correlation_id: msg.take_correlation_id(),
                status,
            }),
        });

        sender
            .send(
                context.source_connection_id().into(),
                IntoBytes::<component::ComponentMessage>::into_bytes(response)?,
            )
            .map_err(|(recipient, msg)| DispatchError::NetworkSendError((recipient.into(), msg)))?;

        Ok(())
    }
}

pub struct ServiceDisconnectRequestHandler {
    service_instances: Box<dyn ServiceInstances + Send>,
}

impl ServiceDisconnectRequestHandler {
    /// Construct a new handler with a given service instances implementation.
    pub fn new(service_instances: Box<dyn ServiceInstances + Send>) -> Self {
        Self { service_instances }
    }
}

impl Handler for ServiceDisconnectRequestHandler {
    type Source = ConnectionId;
    type MessageType = service::ServiceMessageType;
    type Message = service::SMDisconnectRequest;

    fn match_type(&self) -> Self::MessageType {
        service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let service_id: &ServiceId = context.get_parent_context().ok_or_else(|| {
            DispatchError::HandleError(
                "Service Disconnect Request not provided with service ID from envelope.".into(),
            )
        })?;
        let status = match self
            .service_instances
            .remove_service_instance(service_id.clone(), context.source_connection_id().into())
        {
            Ok(()) => DisconnectResponseStatus::Ok,
            Err(ServiceRemoveInstanceError::NotRegistered) => {
                DisconnectResponseStatus::ServiceNotRegistered(format!(
                    "Service {} is not registered",
                    service_id
                ))
            }
            Err(ServiceRemoveInstanceError::NotInCircuit) => {
                DisconnectResponseStatus::ServiceNotInCircuitRegistry(format!(
                    "Service {} is not allowed in circuit {}",
                    service_id.service_id(),
                    service_id.circuit()
                ))
            }
            Err(ServiceRemoveInstanceError::CircuitDoesNotExist) => {
                DisconnectResponseStatus::CircuitDoesNotExist(format!(
                    "Circuit {} does not exist",
                    service_id.circuit()
                ))
            }
            Err(err @ ServiceRemoveInstanceError::InternalError { .. }) => {
                error!("Unable to register service {}: {}", service_id, err);

                DisconnectResponseStatus::InternalError("An internal error has occurred".into())
            }
        };

        let response = ComponentMessage::Service(ServiceMessage {
            circuit: service_id.circuit().to_string(),
            service_id: service_id.service_id().to_string(),
            payload: ServiceMessagePayload::DisconnectResponse(ServiceDisconnectResponse {
                correlation_id: msg.take_correlation_id(),
                status,
            }),
        });

        sender
            .send(
                context.source_connection_id().into(),
                IntoBytes::<component::ComponentMessage>::into_bytes(response)?,
            )
            .map_err(|(recipient, msg)| DispatchError::NetworkSendError((recipient.into(), msg)))?;

        Ok(())
    }
}

/// Dispatch handler for the `ServiceProcessorMessage` messages.
///
/// The messages received by this handler are forwarded to other parts of the system or the network
/// by the provided `ServiceMessageForwarder`.
pub struct ServiceProcessorMessageHandler {
    msg_forwarder: Box<dyn ServiceMessageForwarder + Send>,
}

impl ServiceProcessorMessageHandler {
    /// Construct a new handler with a given service message forwarder.
    pub fn new(msg_forwarder: Box<dyn ServiceMessageForwarder + Send>) -> Self {
        Self { msg_forwarder }
    }
}

impl Handler for ServiceProcessorMessageHandler {
    type Source = ConnectionId;
    type MessageType = service::ServiceMessageType;
    type Message = service::ServiceProcessorMessage;

    fn match_type(&self) -> Self::MessageType {
        service::ServiceMessageType::SM_SERVICE_PROCESSOR_MESSAGE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let service_id: &ServiceId = context.get_parent_context().ok_or_else(|| {
            DispatchError::HandleError(
                "Service Disconnect Request not provided with service ID from envelope.".into(),
            )
        })?;

        let service_msg: ServiceProcessorMessage = msg.into_native()?;
        let correlation_id = service_msg.correlation_id.clone();
        match self.msg_forwarder.forward(service_id, service_msg) {
            Ok(ForwardResult::Sent) => Ok(()),
            Ok(ForwardResult::LocalReReroute(component_id, msg)) => {
                let local_fwd = ComponentMessage::Service(ServiceMessage {
                    circuit: service_id.circuit().to_string(),
                    service_id: msg.recipient.clone(),
                    payload: ServiceMessagePayload::ServiceProcessorMessage(msg),
                });
                sender
                    .send(
                        component_id.into(),
                        IntoBytes::<component::ComponentMessage>::into_bytes(local_fwd)?,
                    )
                    .map_err(|(recipient, msg)| {
                        DispatchError::NetworkSendError((recipient.into(), msg))
                    })
            }
            Err(err) => {
                let (error_kind, error_message) = match err {
                    ServiceForwardingError::SenderNotRegistered => (
                        ErrorKind::SenderNotInDirectory,
                        "Sender is not registered".to_string(),
                    ),
                    ServiceForwardingError::RecipientNotRegistered => (
                        ErrorKind::RecipientNotInDirectory,
                        "Recipient is not registered".to_string(),
                    ),
                    ServiceForwardingError::SenderNotInCircuit => (
                        ErrorKind::SenderNotInCircuit,
                        "Sender is not in the specified circuit".to_string(),
                    ),
                    ServiceForwardingError::RecipientNotInCircuit => (
                        ErrorKind::RecipientNotInCircuit,
                        "Recipient is not the specified circuit".to_string(),
                    ),
                    ServiceForwardingError::CircuitDoesNotExist => (
                        ErrorKind::CircuitDoesNotExist,
                        format!("Circuit {} does not exist", service_id.circuit()),
                    ),
                    internal_err @ ServiceForwardingError::InternalError { .. } => {
                        error!("Unable to forward message: {}", internal_err);

                        (
                            ErrorKind::Internal,
                            "An internal error occurred".to_string(),
                        )
                    }
                };

                let (circuit, service_id) = service_id.clone().into_parts();
                let msg = ComponentMessage::Service(ServiceMessage {
                    circuit,
                    service_id,
                    payload: ServiceMessagePayload::ServiceErrorMessage(ServiceErrorMessage {
                        error_kind,
                        error_message,
                        correlation_id,
                    }),
                });
                sender
                    .send(
                        context.source_id().clone(),
                        IntoBytes::<component::ComponentMessage>::into_bytes(msg)?,
                    )
                    .map_err(|(recipient, msg)| {
                        DispatchError::NetworkSendError((recipient.into(), msg))
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use protobuf::Message;

    use crate::network::dispatch::Dispatcher;
    use crate::protocol::service::ServiceProcessorMessage;

    // Test that service connection request is properly handled and sends a response with an OK
    // status, if the registration is successful.
    #[test]
    fn test_connect_request_ok() {
        let mock_instances = MockServiceInstances::new().with_add_result(Ok(()));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        mock_instances.assert_service_link(
            ServiceId::new("some-circuit".into(), "test-service".into()),
            "service-component".into(),
        );
        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(service::SMConnectResponse_Status::OK, msg.get_status());
                assert!(msg.get_error_message().is_empty());
            },
        );
    }

    // Test that the service connection request is properly handled and sends a response with an
    // ERROR_NOT_AN_ALLOWED_NODE, if the registration returns the error NotAllowed.
    #[test]
    fn test_connect_request_not_allowed() {
        let mock_instances =
            MockServiceInstances::new().with_add_result(Err(ServiceAddInstanceError::NotAllowed));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMConnectResponse_Status::ERROR_NOT_AN_ALLOWED_NODE,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that the service connection request is properly handled and sends a response with an
    // ERROR_SERVICE_ALREADY_REGISTERED, if the registration returns the error AlreadyRegistered.
    #[test]
    fn test_connect_request_already_registered() {
        let mock_instances = MockServiceInstances::new()
            .with_add_result(Err(ServiceAddInstanceError::AlreadyRegistered));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMConnectResponse_Status::ERROR_SERVICE_ALREADY_REGISTERED,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that the service connection request is properly handled and sends a response with an
    // ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY, if the registration returns the error NotInCircuit.
    #[test]
    fn test_connect_request_not_in_circuit() {
        let mock_instances =
            MockServiceInstances::new().with_add_result(Err(ServiceAddInstanceError::NotInCircuit));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMConnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that the service connection request is properly handled and sends a response with an
    // ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY, if the registration returns the error NotInCircuit.
    #[test]
    fn test_connect_request_circuit_does_not_exist() {
        let mock_instances = MockServiceInstances::new()
            .with_add_result(Err(ServiceAddInstanceError::CircuitDoesNotExist));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMConnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that the service connection request is properly handled and sends a response with an
    // ERROR_INTERNAL_ERROR, if the registration returns the error InternalError.
    #[test]
    fn test_connect_request_internal_error() {
        let mock_instances = MockServiceInstances::new().with_add_result(Err(
            ServiceAddInstanceError::InternalError {
                context: "Some error".into(),
                source: None,
            },
        ));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let connect_request_handler =
            ServiceConnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(connect_request_handler));

        let mut connect_req = service::SMConnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_CONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_CONNECT_RESPONSE,
            |msg: service::SMConnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMConnectResponse_Status::ERROR_INTERNAL_ERROR,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that service disconnection request is properly handled and sends a response with an OK
    // status, if the deregistration is successful.
    #[test]
    fn test_disconnect_request_ok() {
        let mock_instances = MockServiceInstances::new().with_remove_result(Ok(()));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(disconnect_request_handler));

        let mut connect_req = service::SMDisconnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_DISCONNECT_RESPONSE,
            |msg: service::SMDisconnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(service::SMDisconnectResponse_Status::OK, msg.get_status());
                assert!(msg.get_error_message().is_empty());
            },
        );
    }

    // Test that service disconnection request is properly handled and sends a response with an
    // ERROR_SERVICE_NOT_REGISTERED status, if the deregistration is fails with a NotRegistered
    // error.
    #[test]
    fn test_disconnect_request_not_registered() {
        let mock_instances = MockServiceInstances::new()
            .with_remove_result(Err(ServiceRemoveInstanceError::NotRegistered));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(disconnect_request_handler));

        let mut connect_req = service::SMDisconnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_DISCONNECT_RESPONSE,
            |msg: service::SMDisconnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMDisconnectResponse_Status::ERROR_SERVICE_NOT_REGISTERED,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that service disconnection request is properly handled and sends a response with an
    // ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY status, if the deregistration is fails with a
    // NotInCircuit error.
    #[test]
    fn test_disconnect_request_not_in_circuit() {
        let mock_instances = MockServiceInstances::new()
            .with_remove_result(Err(ServiceRemoveInstanceError::NotInCircuit));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(disconnect_request_handler));

        let mut connect_req = service::SMDisconnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_DISCONNECT_RESPONSE,
            |msg: service::SMDisconnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMDisconnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that service disconnection request is properly handled and sends a response with an
    // ERROR_CIRCUIT_DOES_NOT_EXIST status, if the deregistration is fails with a
    // CircuitDoesNotExist error.
    #[test]
    fn test_disconnect_request_circuit_does_not_exist() {
        let mock_instances = MockServiceInstances::new()
            .with_remove_result(Err(ServiceRemoveInstanceError::CircuitDoesNotExist));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(disconnect_request_handler));

        let mut connect_req = service::SMDisconnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_DISCONNECT_RESPONSE,
            |msg: service::SMDisconnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMDisconnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that service disconnection request is properly handled and sends a response with an
    // ERROR_INTERNAL_ERROR status, if the deregistration is fails with a InternalError error.
    #[test]
    fn test_disconnect_request_internal_error() {
        let mock_instances = MockServiceInstances::new().with_remove_result(Err(
            ServiceRemoveInstanceError::InternalError {
                context: "An error".into(),
                source: None,
            },
        ));

        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(Box::new(mock_instances.clone()));
        dispatcher.set_handler(Box::new(disconnect_request_handler));

        let mut connect_req = service::SMDisconnectRequest::new();
        connect_req.set_correlation_id("test-correlation-id".into());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_DISCONNECT_REQUEST,
                connect_req.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("some-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (connection_id, msg_bytes) = mock_sender
            .pop_sent()
            .expect("A message should have been sent");

        assert_eq!(ConnectionId::from("service-component"), connection_id);
        assert_service_msg(
            &msg_bytes,
            service::ServiceMessageType::SM_SERVICE_DISCONNECT_RESPONSE,
            |msg: service::SMDisconnectResponse| {
                assert_eq!("test-correlation-id", msg.get_correlation_id());
                assert_eq!(
                    service::SMDisconnectResponse_Status::ERROR_INTERNAL_ERROR,
                    msg.get_status()
                );
                assert!(!msg.get_error_message().is_empty());
            },
        );
    }

    // Test that a service processor message is properly forwarded through the provided message
    // forwarding instance.
    #[test]
    fn test_service_processor_message() {
        let message_forwarder = MockServiceMessageForwarder::default();
        let mock_sender = MockMessageSender::default();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender));

        let service_processor_msg_handler =
            ServiceProcessorMessageHandler::new(Box::new(message_forwarder.clone()));
        dispatcher.set_handler(Box::new(service_processor_msg_handler));

        let mut service_processor_msg = service::ServiceProcessorMessage::new();
        service_processor_msg.set_recipient("target-peer".into());
        service_processor_msg.set_sender("source-peer".into());
        service_processor_msg.set_payload(b"service-bytes".to_vec());

        dispatcher
            .dispatch_with_parent_context(
                "service-component".into(),
                &service::ServiceMessageType::SM_SERVICE_PROCESSOR_MESSAGE,
                service_processor_msg.write_to_bytes().unwrap(),
                Box::new(ServiceId::new("test-circuit".into(), "test-service".into())),
            )
            .expect("unable to dispatch message");

        let (service_id, msg) = message_forwarder
            .pop_forwarded()
            .expect("a message was not fowarded");

        assert_eq!(
            ServiceId::new("test-circuit".into(), "test-service".into()),
            service_id
        );
        assert_eq!("target-peer", &msg.recipient);
        assert_eq!("source-peer", &msg.sender);
        assert_eq!(b"service-bytes".to_vec(), msg.payload);
    }

    #[derive(Clone, Default)]
    struct MockServiceInstances {
        add_result: Arc<Mutex<Option<Result<(), ServiceAddInstanceError>>>>,
        remove_result: Arc<Mutex<Option<Result<(), ServiceRemoveInstanceError>>>>,
        instances: Arc<Mutex<HashMap<ServiceId, String>>>,
    }

    impl MockServiceInstances {
        fn new() -> Self {
            MockServiceInstances::default()
        }

        fn with_add_result(self, result: Result<(), ServiceAddInstanceError>) -> Self {
            self.add_result
                .lock()
                .expect("test lock was poisoned")
                .replace(result);

            self
        }

        fn with_remove_result(self, result: Result<(), ServiceRemoveInstanceError>) -> Self {
            self.remove_result
                .lock()
                .expect("test lock was poisoned")
                .replace(result);

            self
        }

        fn assert_service_link(&self, service_id: ServiceId, component_id: String) {
            assert_eq!(
                Some(&component_id),
                self.instances
                    .lock()
                    .expect("test lock was poisoned")
                    .get(&service_id)
            )
        }
    }

    impl ServiceInstances for MockServiceInstances {
        fn add_service_instance(
            &self,
            service_id: ServiceId,
            component_id: String,
        ) -> Result<(), ServiceAddInstanceError> {
            let res = self
                .add_result
                .lock()
                .expect("test lock was poisoned")
                .take()
                .expect(
                    "Unexpected second call to add_service_instance without resetting the result",
                );

            if res.is_ok() {
                self.instances
                    .lock()
                    .expect("test lock was poisoned")
                    .insert(service_id, component_id);
            }

            res
        }

        fn remove_service_instance(
            &self,
            _service_id: ServiceId,
            _component_id: String,
        ) -> Result<(), ServiceRemoveInstanceError> {
            self.remove_result
                .lock()
                .expect("test lock was poisoned")
                .take()
                .expect("Unexpected second call to remove_service_instance without resetting the result")
        }
    }

    #[derive(Clone, Default)]
    struct MockMessageSender {
        messages: Arc<Mutex<VecDeque<(ConnectionId, Vec<u8>)>>>,
    }

    impl MockMessageSender {
        fn pop_sent(&self) -> Option<(ConnectionId, Vec<u8>)> {
            self.messages
                .lock()
                .expect("test sender lock was poisoned")
                .pop_front()
        }
    }

    impl MessageSender<ConnectionId> for MockMessageSender {
        fn send(
            &self,
            recipient: ConnectionId,
            message: Vec<u8>,
        ) -> Result<(), (ConnectionId, Vec<u8>)> {
            self.messages
                .lock()
                .expect("test sender lock was poisoned")
                .push_back((recipient, message));

            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct MockServiceMessageForwarder {
        messages: Arc<Mutex<VecDeque<(ServiceId, ServiceProcessorMessage)>>>,
    }

    impl MockServiceMessageForwarder {
        fn pop_forwarded(&self) -> Option<(ServiceId, ServiceProcessorMessage)> {
            self.messages
                .lock()
                .expect("test forwarder lock was poisoned")
                .pop_front()
        }
    }

    impl ServiceMessageForwarder for MockServiceMessageForwarder {
        fn forward(
            &self,
            service_id: &ServiceId,
            service_msg: ServiceProcessorMessage,
        ) -> Result<ForwardResult, ServiceForwardingError> {
            self.messages
                .lock()
                .expect("test sender lock was poisoned")
                .push_back((service_id.clone(), service_msg));

            Ok(ForwardResult::Sent)
        }
    }

    fn assert_service_msg<M: protobuf::Message, F: Fn(M)>(
        msg_bytes: &[u8],
        expected_service_msg_type: service::ServiceMessageType,
        detail_assertions: F,
    ) {
        let component_message: component::ComponentMessage =
            protobuf::parse_from_bytes(msg_bytes).unwrap();
        let service_msg: service::ServiceMessage =
            protobuf::parse_from_bytes(component_message.get_payload()).unwrap();
        assert_eq!(expected_service_msg_type, service_msg.get_message_type(),);
        let service_msg_paylaod: M = protobuf::parse_from_bytes(service_msg.get_payload()).unwrap();

        detail_assertions(service_msg_paylaod);
    }
}
