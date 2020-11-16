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

//! Structs for building proposed circuits

use crate::admin::messages::is_valid_circuit_id;
use crate::error::InvalidStateError;
use crate::protos::admin;

use super::{
    AuthorizationType, DurabilityType, PersistenceType, ProposedNode, ProposedService, RouteType,
};

/// Native representation of a circuit that is being proposed in a proposal
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedCircuit {
    circuit_id: String,
    roster: Vec<ProposedService>,
    members: Vec<ProposedNode>,
    authorization_type: AuthorizationType,
    persistence: PersistenceType,
    durability: DurabilityType,
    routes: RouteType,
    circuit_management_type: String,
    application_metadata: Vec<u8>,
    comments: String,
}

impl ProposedCircuit {
    /// Returns the ID of the circuit
    pub fn circuit_id(&self) -> &str {
        &self.circuit_id
    }

    /// Returns the list of service that are in the circuit
    pub fn roster(&self) -> &[ProposedService] {
        &self.roster
    }

    /// Returns the list of node IDs that are in the circuit
    pub fn members(&self) -> &[ProposedNode] {
        &self.members
    }

    /// Returns the authorization type of the circuit
    pub fn authorization_type(&self) -> &AuthorizationType {
        &self.authorization_type
    }

    /// Returns the persistence type type of the circuit
    pub fn persistence(&self) -> &PersistenceType {
        &self.persistence
    }

    /// Returns the durability type of the circuit
    pub fn durability(&self) -> &DurabilityType {
        &self.durability
    }

    /// Returns the route type of the circuit
    pub fn routes(&self) -> &RouteType {
        &self.routes
    }

    /// Returns the mangement type of the circuit
    pub fn circuit_management_type(&self) -> &str {
        &self.circuit_management_type
    }

    pub fn application_metadata(&self) -> &[u8] {
        &self.application_metadata
    }

    /// Returns the mangement type of the circuit
    pub fn comments(&self) -> &str {
        &self.comments
    }

    pub fn from_proto(mut proto: admin::Circuit) -> Result<Self, InvalidStateError> {
        let authorization_type = match proto.get_authorization_type() {
            admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION => AuthorizationType::Trust,
            admin::Circuit_AuthorizationType::UNSET_AUTHORIZATION_TYPE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `authorization_type`".to_string(),
                ));
            }
        };

        let persistence = match proto.get_persistence() {
            admin::Circuit_PersistenceType::ANY_PERSISTENCE => PersistenceType::Any,
            admin::Circuit_PersistenceType::UNSET_PERSISTENCE_TYPE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `persistence type`".to_string(),
                ));
            }
        };

        let durability = match proto.get_durability() {
            admin::Circuit_DurabilityType::NO_DURABILITY => DurabilityType::NoDurability,
            admin::Circuit_DurabilityType::UNSET_DURABILITY_TYPE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `durability type`".to_string(),
                ));
            }
        };

        let routes = match proto.get_routes() {
            admin::Circuit_RouteType::ANY_ROUTE => RouteType::Any,
            admin::Circuit_RouteType::UNSET_ROUTE_TYPE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `route type`".to_string(),
                ));
            }
        };

        Ok(Self {
            circuit_id: proto.take_circuit_id(),
            roster: proto
                .take_roster()
                .into_iter()
                .map(ProposedService::from_proto)
                .collect::<Result<Vec<ProposedService>, InvalidStateError>>()?,
            members: proto
                .take_members()
                .into_iter()
                .map(ProposedNode::from_proto)
                .collect::<Vec<ProposedNode>>(),
            authorization_type,
            persistence,
            durability,
            routes,
            circuit_management_type: proto.take_circuit_management_type(),
            application_metadata: proto.take_application_metadata(),
            comments: proto.take_comments(),
        })
    }

    pub fn into_proto(self) -> admin::Circuit {
        let mut circuit = admin::Circuit::new();

        circuit.set_circuit_id(self.circuit_id);
        circuit.set_roster(protobuf::RepeatedField::from_vec(
            self.roster
                .into_iter()
                .map(ProposedService::into_proto)
                .collect(),
        ));
        circuit.set_members(protobuf::RepeatedField::from_vec(
            self.members
                .into_iter()
                .map(ProposedNode::into_proto)
                .collect(),
        ));

        circuit.set_circuit_management_type(self.circuit_management_type);
        circuit.set_application_metadata(self.application_metadata);
        circuit.set_comments(self.comments);

        match self.authorization_type {
            AuthorizationType::Trust => {
                circuit
                    .set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
            }
        };

        match self.persistence {
            PersistenceType::Any => {
                circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
            }
        };
        match self.durability {
            DurabilityType::NoDurability => {
                circuit.set_durability(admin::Circuit_DurabilityType::NO_DURABILITY);
            }
        };

        match self.routes {
            RouteType::Any => circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE),
        };

        circuit
    }
}

/// Builder to be used to build a `ProposedCircuit` which will be included in a `CircuitProposal`
#[derive(Default, Clone)]
pub struct ProposedCircuitBuilder {
    circuit_id: Option<String>,
    roster: Option<Vec<ProposedService>>,
    members: Option<Vec<ProposedNode>>,
    authorization_type: Option<AuthorizationType>,
    persistence: Option<PersistenceType>,
    durability: Option<DurabilityType>,
    routes: Option<RouteType>,
    circuit_management_type: Option<String>,
    application_metadata: Option<Vec<u8>>,
    comments: Option<String>,
}

impl ProposedCircuitBuilder {
    /// Creates a new proposed circuit builder
    pub fn new() -> Self {
        ProposedCircuitBuilder::default()
    }

    // Returns the circuit ID in the builder
    pub fn circuit_id(&self) -> Option<String> {
        self.circuit_id.clone()
    }

    /// Returns the list of services in the builder
    pub fn roster(&self) -> Option<Vec<ProposedService>> {
        self.roster.clone()
    }

    /// Returns the list of node IDs in the builder
    pub fn members(&self) -> Option<Vec<ProposedNode>> {
        self.members.clone()
    }

    /// Returns the authorizationtype in the builder
    pub fn authorization_type(&self) -> Option<AuthorizationType> {
        self.authorization_type.clone()
    }

    /// Returns the persistence type in the builder
    pub fn persistence(&self) -> Option<PersistenceType> {
        self.persistence.clone()
    }

    /// Returns the durability type in the builder
    pub fn durability(&self) -> Option<DurabilityType> {
        self.durability.clone()
    }

    /// Returns the routing type in the builder
    pub fn routes(&self) -> Option<RouteType> {
        self.routes.clone()
    }

    /// Returns the circuit management type in the builder
    pub fn circuit_management_type(&self) -> Option<String> {
        self.circuit_management_type.clone()
    }

    /// Returns the appplication metdata in the builder
    pub fn application_metadata(&self) -> Option<Vec<u8>> {
        self.application_metadata.clone()
    }

    /// Returns the comments describing the circuit proposal in the builder
    pub fn comments(&self) -> Option<String> {
        self.comments.clone()
    }

    /// Sets the circuit ID
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit
    pub fn with_circuit_id(mut self, circuit_id: &str) -> ProposedCircuitBuilder {
        self.circuit_id = Some(circuit_id.into());
        self
    }

    /// Sets the list of services that are included in the circuit
    ///
    /// # Arguments
    ///
    ///  * `services` - List of proposed services
    pub fn with_roster(mut self, services: &[ProposedService]) -> ProposedCircuitBuilder {
        self.roster = Some(services.into());
        self
    }

    /// Sets the list of nodes that are included in the circuit
    ///
    /// # Arguments
    ///
    ///  * `members` - List of proposed nodes
    pub fn with_members(mut self, members: &[ProposedNode]) -> ProposedCircuitBuilder {
        self.members = Some(members.into());
        self
    }

    /// Sets the authorizationtype
    ///
    /// # Arguments
    ///
    ///  * `auth` - The authorization type for the circuit
    pub fn with_authorization_type(mut self, auth: &AuthorizationType) -> ProposedCircuitBuilder {
        self.authorization_type = Some(auth.clone());
        self
    }

    /// Sets the persistence type
    ///
    /// # Arguments
    ///
    ///  * `persistence` - The persistence type for the circuit
    pub fn with_persistence(mut self, persistence: &PersistenceType) -> ProposedCircuitBuilder {
        self.persistence = Some(persistence.clone());
        self
    }

    /// Sets the durability type
    ///
    /// # Arguments
    ///
    ///  * `durability` - The durability type for the circuit
    pub fn with_durability(mut self, durability: &DurabilityType) -> ProposedCircuitBuilder {
        self.durability = Some(durability.clone());
        self
    }

    /// Sets the routes type
    ///
    /// # Arguments
    ///
    ///  * `routes` - The routes type for the circuit
    pub fn with_routes(mut self, route_type: &RouteType) -> ProposedCircuitBuilder {
        self.routes = Some(route_type.clone());
        self
    }

    /// Sets the circuit managment type
    ///
    /// # Arguments
    ///
    ///  * `circuit_management_type` - The circuit_management_type for the circuit
    pub fn with_circuit_management_type(
        mut self,
        circuit_management_type: &str,
    ) -> ProposedCircuitBuilder {
        self.circuit_management_type = Some(circuit_management_type.into());
        self
    }

    /// Sets the application metadata
    ///
    /// # Arguments
    ///
    ///  * `application_metadata` - The application_metadata for the proposed circuit
    pub fn with_application_metadata(
        mut self,
        application_metadata: &[u8],
    ) -> ProposedCircuitBuilder {
        self.application_metadata = Some(application_metadata.into());
        self
    }

    /// Sets the comments
    ///
    /// # Arguments
    ///
    ///  * `comments` - The comments describing the purpose of the proposed circuit
    pub fn with_comments(mut self, comments: &str) -> ProposedCircuitBuilder {
        self.comments = Some(comments.into());
        self
    }

    /// Builds a `ProposedCircuit`
    ///
    /// Returns an error if the circuit ID, roster, members or circuit management
    /// type are not set.
    pub fn build(self) -> Result<ProposedCircuit, InvalidStateError> {
        let circuit_id = match self.circuit_id {
            Some(circuit_id) if is_valid_circuit_id(&circuit_id) => circuit_id,
            Some(circuit_id) => {
                return Err(InvalidStateError::with_message(format!(
                    "circuit_id is invalid ({}): must be an 11 character string composed of two, \
                     5 character base62 strings joined with a '-' (example: abcDE-F0123)",
                    circuit_id,
                )))
            }
            None => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `circuit_id`".to_string(),
                ))
            }
        };

        let mut roster = self.roster.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `roster`".to_string())
        })?;

        roster.sort_by_key(|service| service.service_id().to_string());

        let mut members = self.members.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `members`".to_string())
        })?;

        members.sort_by_key(|node| node.node_id().to_string());

        let authorization_type = self
            .authorization_type
            .unwrap_or_else(|| AuthorizationType::Trust);

        let persistence = self.persistence.unwrap_or_else(PersistenceType::default);

        let durability = self
            .durability
            .unwrap_or_else(|| DurabilityType::NoDurability);

        let routes = self.routes.unwrap_or_else(RouteType::default);

        let circuit_management_type = self.circuit_management_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `circuit_management_type`".to_string(),
            )
        })?;

        let application_metadata = self.application_metadata.unwrap_or_default();

        let comments = self.comments.unwrap_or_default();

        let create_circuit_message = ProposedCircuit {
            circuit_id,
            roster,
            members,
            authorization_type,
            persistence,
            durability,
            routes,
            circuit_management_type,
            application_metadata,
            comments,
        };

        Ok(create_circuit_message)
    }
}
