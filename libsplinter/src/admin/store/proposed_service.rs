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

//! Structs for building proposed services
use crate::admin::messages::is_valid_service_id;
use crate::error::InvalidStateError;
use crate::protos::admin;

/// Native representation of a service that is a part of a proposed circuit
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedService {
    service_id: String,
    service_type: String,
    node_id: String,
    arguments: Vec<(String, String)>,
}

impl ProposedService {
    /// Returns the ID of the proposed service
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Returns the service type of the proposed service
    pub fn service_type(&self) -> &str {
        &self.service_type
    }

    /// Returns the node the proposed service can run on
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the list of key/value arugments for the  proposed service
    pub fn arguments(&self) -> &[(String, String)] {
        &self.arguments
    }

    pub fn into_proto(self) -> admin::SplinterService {
        let mut proto = admin::SplinterService::new();
        proto.set_service_id(self.service_id);
        proto.set_service_type(self.service_type);
        proto.set_allowed_nodes(protobuf::RepeatedField::from_vec(vec![self.node_id]));
        proto.set_arguments(protobuf::RepeatedField::from_vec(
            self.arguments
                .into_iter()
                .map(|(k, v)| {
                    let mut argument = admin::SplinterService_Argument::new();
                    argument.set_key(k);
                    argument.set_value(v);
                    argument
                })
                .collect(),
        ));

        proto
    }

    pub fn from_proto(mut proto: admin::SplinterService) -> Result<Self, InvalidStateError> {
        Ok(Self {
            service_id: proto.take_service_id(),
            service_type: proto.take_service_type(),
            node_id: proto
                .take_allowed_nodes()
                .get(0)
                .ok_or_else(|| {
                    InvalidStateError::with_message(
                        "unable to build, missing field: `node_id`".to_string(),
                    )
                })?
                .to_string(),
            arguments: proto
                .take_arguments()
                .into_iter()
                .map(|mut argument| (argument.take_key(), argument.take_value()))
                .collect(),
        })
    }
}

/// Builder for creating a `ProposedService`
#[derive(Default, Clone)]
pub struct ProposedServiceBuilder {
    service_id: Option<String>,
    service_type: Option<String>,
    node_id: Option<String>,
    arguments: Option<Vec<(String, String)>>,
}

impl ProposedServiceBuilder {
    /// Creates a new `ProposedServiceBuilder`
    pub fn new() -> Self {
        ProposedServiceBuilder::default()
    }

    /// Returns the service specific service ID
    pub fn service_id(&self) -> Option<String> {
        self.service_id.clone()
    }

    /// Returns the service type
    pub fn service_type(&self) -> Option<String> {
        self.service_type.clone()
    }

    /// Returns the node ID the service can connect to
    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
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
    pub fn with_service_id(mut self, service_id: &str) -> ProposedServiceBuilder {
        self.service_id = Some(service_id.into());
        self
    }

    /// Sets the service type
    ///
    /// # Arguments
    ///
    ///  * `service_type` - The service type of the service
    pub fn with_service_type(mut self, service_type: &str) -> ProposedServiceBuilder {
        self.service_type = Some(service_type.into());
        self
    }

    /// Sets the node ID the service is allowed to connect to
    ///
    /// # Arguments
    ///
    ///  * `node_id` - A node ID of the node the service can connect to
    pub fn with_node_id(mut self, node_id: &str) -> ProposedServiceBuilder {
        self.node_id = Some(node_id.into());
        self
    }

    /// Sets the service arguments
    ///
    /// # Arguments
    ///
    ///  * `arguments` - A list of key-value pairs for the arguments for the service
    pub fn with_arguments(mut self, arguments: &[(String, String)]) -> ProposedServiceBuilder {
        self.arguments = Some(arguments.to_vec());
        self
    }

    /// Builds the `ProposedService`
    ///
    /// Returns an error if the service ID, service_type, or allowed nodes is not set
    pub fn build(self) -> Result<ProposedService, InvalidStateError> {
        let service_id = match self.service_id {
            Some(service_id) if is_valid_service_id(&service_id) => service_id,
            Some(service_id) => {
                return Err(InvalidStateError::with_message(format!(
                    "service_id is invalid ({}): must be a 4 character base62 string",
                    service_id,
                )))
            }
            None => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `service_id`".to_string(),
                ))
            }
        };

        let service_type = self.service_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_type`".to_string(),
            )
        })?;

        let node_id = self.node_id.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `node_id`".to_string())
        })?;

        let mut arguments = self.arguments.unwrap_or_default();

        arguments.sort();

        let service = ProposedService {
            service_id,
            service_type,
            node_id,
            arguments,
        };

        Ok(service)
    }
}
