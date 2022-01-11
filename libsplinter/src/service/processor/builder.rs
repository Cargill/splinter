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

//! Builder for constructing new service processors.

use crate::error::InvalidStateError;
use crate::service::Service;
use crate::transport::Connection;

use super::ServiceProcessor;

const DEFAULT_INCOMING_CAPACITY: usize = 8;
const DEFAULT_OUTGOING_CAPACITY: usize = 8;
const DEFAULT_CHANNEL_CAPACITY: usize = 8;

/// Builds new ServiceProcessors.
#[derive(Default)]
pub struct ServiceProcessorBuilder {
    connection: Option<Box<dyn Connection>>,
    circuit: Option<String>,
    incoming_capacity: Option<usize>,
    outgoing_capacity: Option<usize>,
    channel_capacity: Option<usize>,
    services: Vec<Box<dyn Service>>,
}

impl ServiceProcessorBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the connection for receiving service messages by the resulting ServiceProcessor.
    ///
    /// This field is required to construct the final ServiceProcessor.
    pub fn with_connection(mut self, connection: Box<dyn Connection>) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Sets the circuit associated with all of the services in this processor.
    ///
    /// This field is required to construct the final ServiceProcessor.
    pub fn with_circuit(mut self, circuit: String) -> Self {
        self.circuit = Some(circuit);
        self
    }

    /// Sets the incoming message capacity.
    ///
    /// This limits the amount of messages that may be buffered by the service processor before
    /// blocking new messages.
    pub fn with_incoming_capacity(mut self, incoming_capacity: usize) -> Self {
        self.incoming_capacity = Some(incoming_capacity);
        self
    }

    /// Sets the outgoing message capacity.
    ///
    /// This limits the amount of messages that may be buffered by the service processor when being
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

    /// Adds a service to be managed by the resulting ServiceProcessor.
    ///
    /// This function may be called more than once to add additional services.  It must be called
    /// at least once to construct a valid service processor.
    pub fn with_service(mut self, service: Box<dyn Service>) -> Self {
        self.services.push(service);

        self
    }

    /// Construct the ServiceProcessor.
    ///
    /// # Errors
    ///
    /// Returns an InvalidStateError, if any required fields are missing.
    pub fn build(self) -> Result<ServiceProcessor, InvalidStateError> {
        let connection = self.connection.ok_or_else(|| {
            InvalidStateError::with_message("A service processor requires a connection".into())
        })?;

        let circuit = self.circuit.ok_or_else(|| {
            InvalidStateError::with_message("A service processor requires a circuit".into())
        })?;

        let incoming_capacity = self.incoming_capacity.unwrap_or(DEFAULT_INCOMING_CAPACITY);
        let outgoing_capacity = self.outgoing_capacity.unwrap_or(DEFAULT_OUTGOING_CAPACITY);
        let channel_capacity = self.channel_capacity.unwrap_or(DEFAULT_CHANNEL_CAPACITY);

        if self.services.is_empty() {
            return Err(InvalidStateError::with_message(
                "At least one service is required by a service processor".into(),
            ));
        }

        let mut processor = ServiceProcessor::new(
            connection,
            circuit,
            incoming_capacity,
            outgoing_capacity,
            channel_capacity,
        )
        .map_err(|e| InvalidStateError::with_message(e.to_string()))?;

        for service in self.services.into_iter() {
            processor
                .add_service(service)
                .map_err(|e| InvalidStateError::with_message(e.to_string()))?;
        }

        Ok(processor)
    }
}
