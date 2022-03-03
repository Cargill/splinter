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

use std::any::Any;

use crate::error::InternalError;
use crate::service::{
    ServiceDestroyError, ServiceError, ServiceNetworkRegistry,
    ServiceStartError, ServiceStopError,
};

use super::ServiceMessageContext;

/// A Service provides message handling for a given service type.
pub trait ServiceInstance: Send {
    /// This service's ID
    ///
    /// This ID must be unique within the context of a circuit, but not necessarily unique within
    /// the context of a splinter node, as a whole.
    fn service_id(&self) -> &str;

    /// This service's type
    ///
    /// A service type broadly identifies the kinds of messages that this service handles or emits.
    fn service_type(&self) -> &str;

    /// Starts the service
    ///
    /// At start time, the service should register itself with the network when its ready to
    /// receive messages.
    fn start(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStartError>;

    /// Stops Starts the service
    ///
    /// The service should unregister itself with the network.
    fn stop(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStopError>;

    /// Clean-up any resources before the service is removed.
    /// Consumes the service (which, given the use of dyn traits,
    /// this must take a boxed Service instance).
    fn destroy(self: Box<Self>) -> Result<(), ServiceDestroyError>;

    /// Purge any persistent state maintained by this service.
    fn purge(&mut self) -> Result<(), InternalError>;

    /// Handle any incoming message intended for this service instance.
    ///
    /// Messages recevied by this service are provided in raw bytes.  The format of the service
    fn handle_message(
        &self,
        message_bytes: &[u8],
        message_context: &ServiceMessageContext,
    ) -> Result<(), ServiceError>;

    /// Cast the service as `&dyn Any`.
    ///
    /// This allows for downcasting the `Service` to a specific implementation.
    fn as_any(&self) -> &dyn Any;
}
