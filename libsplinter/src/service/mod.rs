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

//!  Splinter services are a fundamental component of a Splinter network.  They provide the
//!  application-level business logic, abstracted above the underlying network and circuit layers.
//!  The Service API provides a framework for defining services from a Splinter perspective.
//!
//!  Splinter services are effectively message handlers.  They receive the bytes of a message and a
//!  context for the message (including the originating sender).  It is up to the service to parse
//!  the message bytes into the format desired.
//!
//!  A service is identified by an ID, which must be unique within a Splinter circuit.  It also
//!  provides a service type, which indicates what kinds of capabilities the service provides.
//!
//!  There may be more than one service of a given service type on a circuit, though each instance
//!  must continue to have a unique ID.
//!
//!  Services are started and stopped explicitly.  At these times, the service must either register
//!  (at start) or unregister (at stop) itself.  In splinter terms, these two operations connect
//!  and disconnect the service from a circuit, but the Service API keeps the service circuit-
//!  agnostic.
//!
//!  A stand-alone service implementation may be wrapped in a ServiceProcessor, which will manage
//!  lower-level messaging and networking needs to talk to applications that implement Splinter
//!  node capabilities, such as the Splinter daemon.

pub mod error;
mod factory;
#[cfg(feature = "service-id")]
mod id;
pub mod instance;
#[cfg(feature = "service-network")]
pub mod network;
mod processor;
#[cfg(feature = "rest-api-actix-web-1")]
pub mod rest_api;
pub mod validation;

pub use factory::ServiceFactory;
#[cfg(feature = "service-id")]
pub use id::{CircuitId, FullyQualifiedServiceId, ServiceId};
use instance::ServiceMessageContext;
pub use processor::registry::StandardServiceNetworkRegistry;
pub use processor::JoinHandles;
pub use processor::ServiceProcessor;
pub use processor::ServiceProcessorBuilder;
pub use processor::ServiceProcessorShutdownHandle;

pub use error::{
    FactoryCreateError, ServiceConnectionError, ServiceDestroyError, ServiceDisconnectionError,
    ServiceError, ServiceProcessorError, ServiceSendError, ServiceStartError, ServiceStopError,
};

/// The ServiceNetworkRegistry trait provides functions to register and unregister the service on
/// the network.  It does not expose the circuit membership information directly.
pub trait ServiceNetworkRegistry: Send {
    fn connect(
        &self,
        service_id: &str,
    ) -> Result<Box<dyn ServiceNetworkSender>, ServiceConnectionError>;
    fn disconnect(&self, service_id: &str) -> Result<(), ServiceDisconnectionError>;
}

/// The ServiceNetworkSender trait allows a service to send its own messages, such as replies to
/// the original message or forwarding the message to other services on the same circuit.  It does
/// not expose the circuit information directly.
pub trait ServiceNetworkSender: Send {
    /// Send the message bytes to the given recipient (another service)
    fn send(&self, recipient: &str, message: &[u8]) -> Result<(), ServiceSendError>;

    /// Send the message bytes to the given recipient (another service) and await the reply.  This
    /// function blocks until the reply is returned.
    fn send_and_await(&self, recipient: &str, message: &[u8]) -> Result<Vec<u8>, ServiceSendError>;

    /// Send the message bytes back to the origin specified in the given message context.
    fn reply(
        &self,
        message_origin: &ServiceMessageContext,
        message: &[u8],
    ) -> Result<(), ServiceSendError>;

    /// Clone this instance into Boxed, dynamic trait
    fn clone_box(&self) -> Box<dyn ServiceNetworkSender>;

    /// Send the message bytes to the given recipient (another service) with a configurable
    /// message sender
    fn send_with_sender(
        &mut self,
        recipient: &str,
        message: &[u8],
        sender: &str,
    ) -> Result<(), ServiceSendError>;
}

impl Clone for Box<dyn ServiceNetworkSender> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
