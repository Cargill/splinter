// Copyright 2018-2021 Cargill Incorporated
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

//! Builder for constructing new service orchestrators.

use crate::error::InvalidStateError;
use crate::transport::Connection;

use super::runnable::RunnableServiceOrchestrator;
use super::OrchestratableServiceFactory;

const DEFAULT_INCOMING_CAPACITY: usize = 512;
const DEFAULT_OUTGOING_CAPACITY: usize = 512;
const DEFAULT_CHANNEL_CAPACITY: usize = 512;

/// Builds new [RunnableServiceOrchestrator] instances.
#[derive(Default)]
pub struct ServiceOrchestratorBuilder {
    connection: Option<Box<dyn Connection>>,
    incoming_capacity: Option<usize>,
    outgoing_capacity: Option<usize>,
    channel_capacity: Option<usize>,
    service_factories: Vec<Box<dyn OrchestratableServiceFactory>>,
}

impl ServiceOrchestratorBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the connection for receiving service messages by the resulting ServiceOrchestrator.
    ///
    /// This field is required to construct the final ServiceOrchestrator.
    pub fn with_connection(mut self, connection: Box<dyn Connection>) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Sets the incoming message capacity.
    ///
    /// This limits the amount of messages that may be buffered by the service orchestrator before
    /// blocking new messages.
    pub fn with_incoming_capacity(mut self, incoming_capacity: usize) -> Self {
        self.incoming_capacity = Some(incoming_capacity);
        self
    }

    /// Sets the outgoing message capacity.
    ///
    /// This limits the amount of messages that may be buffered by the service orchestrator when being
    /// sent to external connections.
    pub fn with_outgoing_capacity(mut self, outgoing_capacity: usize) -> Self {
        self.outgoing_capacity = Some(outgoing_capacity);
        self
    }

    /// Sets the internal channel capacity.
    ///
    /// This limits the number of messages that may be buffered when passed between the internal
    /// threads.
    pub fn with_channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = Some(channel_capacity);
        self
    }

    /// Adds a service factory which will be used to create service instances.
    ///
    /// This function may be called more than once to add additional service factories.
    pub fn with_service_factory(
        mut self,
        service_factory: Box<dyn OrchestratableServiceFactory>,
    ) -> Self {
        self.service_factories.push(service_factory);

        self
    }

    /// Construct the RunnableServiceOrchestrator.
    ///
    /// # Errors
    ///
    /// Returns an InvalidStateError, if any required fields are missing.
    pub fn build(self) -> Result<RunnableServiceOrchestrator, InvalidStateError> {
        let connection = self.connection.ok_or_else(|| {
            InvalidStateError::with_message("A service orchestrator requires a connection".into())
        })?;

        let incoming_capacity = self.incoming_capacity.unwrap_or(DEFAULT_INCOMING_CAPACITY);
        let outgoing_capacity = self.outgoing_capacity.unwrap_or(DEFAULT_OUTGOING_CAPACITY);
        let channel_capacity = self.channel_capacity.unwrap_or(DEFAULT_CHANNEL_CAPACITY);

        let supported_service_types_vec = self
            .service_factories
            .iter()
            .map(|factory| factory.available_service_types().to_vec())
            .collect::<Vec<Vec<String>>>();

        let mut supported_service_types = vec![];
        for mut service_types in supported_service_types_vec {
            supported_service_types.append(&mut service_types);
        }

        Ok(RunnableServiceOrchestrator {
            connection,
            service_factories: self.service_factories,
            supported_service_types,
            incoming_capacity,
            outgoing_capacity,
            channel_capacity,
        })
    }
}
