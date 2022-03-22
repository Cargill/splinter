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

//! Structs for building services
use std::fmt;

use crate::error::InvalidStateError;
use crate::service::{FullyQualifiedServiceId, ServiceType};

/// Native representation of a service that is a part of circuit
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleService {
    service_id: FullyQualifiedServiceId,
    service_type: ServiceType<'static>,
    arguments: Vec<(String, String)>,
    command: LifecycleCommand,
    status: LifecycleStatus,
}

impl LifecycleService {
    /// Returns the ID of the service
    pub fn service_id(&self) -> &FullyQualifiedServiceId {
        &self.service_id
    }

    /// Returns the service type of the service
    pub fn service_type(&self) -> &ServiceType {
        &self.service_type
    }

    /// Returns the list of key/value arugments for the service
    pub fn arguments(&self) -> &[(String, String)] {
        &self.arguments
    }

    /// Returns lifecycle command of the service
    pub fn command(&self) -> &LifecycleCommand {
        &self.command
    }

    /// Returns lifecycle status of the service
    pub fn status(&self) -> &LifecycleStatus {
        &self.status
    }

    pub fn into_builder(self) -> LifecycleServiceBuilder {
        LifecycleServiceBuilder::new()
            .with_service_id(&self.service_id)
            .with_service_type(&self.service_type)
            .with_arguments(&self.arguments)
            .with_command(&self.command)
            .with_status(&self.status)
    }
}

/// Builder for creating a `LifecycleService`
#[derive(Default, Clone)]
pub struct LifecycleServiceBuilder {
    service_id: Option<FullyQualifiedServiceId>,
    service_type: Option<ServiceType<'static>>,
    arguments: Option<Vec<(String, String)>>,
    command: Option<LifecycleCommand>,
    status: Option<LifecycleStatus>,
}

impl LifecycleServiceBuilder {
    /// Creates a new `LifecycleServiceBuilder`
    pub fn new() -> Self {
        LifecycleServiceBuilder::default()
    }

    /// Returns the service specific service ID
    pub fn service_id(&self) -> Option<FullyQualifiedServiceId> {
        self.service_id.clone()
    }

    /// Returns the service type
    pub fn service_type(&self) -> Option<ServiceType> {
        self.service_type.clone()
    }

    /// Returns lifecycle command of the service
    pub fn command(&self) -> Option<LifecycleCommand> {
        self.command.clone()
    }

    /// Returns lifecycle status of the service
    pub fn status(&self) -> Option<LifecycleStatus> {
        self.status.clone()
    }

    /// Returns the list of arguments for the service
    pub fn arguments(&self) -> Option<Vec<(String, String)>> {
        self.arguments.clone()
    }

    /// Sets the service ID
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The unique service ID for service
    pub fn with_service_id(
        mut self,
        service_id: &FullyQualifiedServiceId,
    ) -> LifecycleServiceBuilder {
        self.service_id = Some(service_id.clone());
        self
    }

    /// Sets the service type
    ///
    /// # Arguments
    ///
    ///  * `service_type` - The service type of the service
    pub fn with_service_type(
        mut self,
        service_type: &ServiceType<'static>,
    ) -> LifecycleServiceBuilder {
        self.service_type = Some(service_type.clone());
        self
    }

    /// Sets the lifecycle command
    ///
    /// # Arguments
    ///
    ///  * `command` - The lifecycle command of the service
    pub fn with_command(mut self, command: &LifecycleCommand) -> LifecycleServiceBuilder {
        self.command = Some(command.clone());
        self
    }

    /// Sets the lifecycle status
    ///
    /// # Arguments
    ///
    ///  * `status` - The lifecycle status of the service
    pub fn with_status(mut self, status: &LifecycleStatus) -> LifecycleServiceBuilder {
        self.status = Some(status.clone());
        self
    }

    /// Sets the service arguments
    ///
    /// # Arguments
    ///
    ///  * `arguments` - A list of key-value pairs for the arguments for the service
    pub fn with_arguments(mut self, arguments: &[(String, String)]) -> LifecycleServiceBuilder {
        self.arguments = Some(arguments.to_vec());
        self
    }

    /// Builds the `LifecycleService`
    ///
    /// Returns an error if the service ID, service_type, or allowed nodes is not set
    pub fn build(self) -> Result<LifecycleService, InvalidStateError> {
        let service_id = self.service_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_id`".to_string(),
            )
        })?;

        let service_type = self.service_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_type`".to_string(),
            )
        })?;

        let status = self.status.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `status`".to_string())
        })?;

        let command = self.command.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `command`".to_string())
        })?;

        let arguments = self.arguments.unwrap_or_default();

        let service = LifecycleService {
            service_id,
            service_type,
            command,
            status,
            arguments,
        };

        Ok(service)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleStatus {
    New,
    Complete,
}

impl fmt::Display for LifecycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleStatus::New => write!(f, "Status: New"),
            LifecycleStatus::Complete => write!(f, "Status: Complete"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleCommand {
    Prepare,
    Finalize,
    Retire,
    Purge,
}

impl fmt::Display for LifecycleCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleCommand::Prepare => write!(f, "Command: Prepare"),
            LifecycleCommand::Finalize => write!(f, "Command: Finalize"),
            LifecycleCommand::Retire => write!(f, "Command: Retire"),
            LifecycleCommand::Purge => write!(f, "Command: Purge"),
        }
    }
}
