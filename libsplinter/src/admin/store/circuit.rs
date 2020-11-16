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

//! Structs for building circuits

use crate::admin::messages::is_valid_circuit_id;
use crate::error::InvalidStateError;

use super::{ProposedCircuit, Service};

/// Native representation of a circuit in state
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circuit {
    id: String,
    roster: Vec<Service>,
    members: Vec<String>,
    authorization_type: AuthorizationType,
    persistence: PersistenceType,
    durability: DurabilityType,
    routes: RouteType,
    circuit_management_type: String,
}

impl Circuit {
    /// Returns the ID of the circuit
    pub fn circuit_id(&self) -> &str {
        &self.id
    }

    /// Returns the list of service that are in the circuit
    pub fn roster(&self) -> &[Service] {
        &self.roster
    }

    /// Returns the list of node IDs that are in the circuit
    pub fn members(&self) -> &[String] {
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
}

/// What type of authorization the circuit requires
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizationType {
    Trust,
}

/// A circuits message persistence strategy
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceType {
    Any,
}

impl Default for PersistenceType {
    fn default() -> Self {
        PersistenceType::Any
    }
}

/// A circuits durability requirement
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurabilityType {
    NoDurability,
}

/// How messages are expected to be routed across a circuit
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteType {
    Any,
}

impl Default for RouteType {
    fn default() -> Self {
        RouteType::Any
    }
}

/// Builder to be used to build a `Circuit`
#[derive(Default, Clone)]
pub struct CircuitBuilder {
    circuit_id: Option<String>,
    roster: Option<Vec<Service>>,
    members: Option<Vec<String>>,
    authorization_type: Option<AuthorizationType>,
    persistence: Option<PersistenceType>,
    durability: Option<DurabilityType>,
    routes: Option<RouteType>,
    circuit_management_type: Option<String>,
}

impl CircuitBuilder {
    /// Creates a new circuit builder
    pub fn new() -> Self {
        CircuitBuilder::default()
    }

    /// Returns the circuit ID in the builder
    pub fn circuit_id(&self) -> Option<String> {
        self.circuit_id.clone()
    }

    /// Returns the list of services in the builder
    pub fn roster(&self) -> Option<Vec<Service>> {
        self.roster.clone()
    }

    /// Returns the list of node IDs in the builder
    pub fn members(&self) -> Option<Vec<String>> {
        self.members.clone()
    }

    /// Returns the authorization type in the builder
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

    /// Sets the circuit ID
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit
    pub fn with_circuit_id(mut self, circuit_id: &str) -> CircuitBuilder {
        self.circuit_id = Some(circuit_id.into());
        self
    }

    /// Sets the list of services that are included in the circuit
    ///
    /// # Arguments
    ///
    ///  * `services` - List of services
    pub fn with_roster(mut self, services: &[Service]) -> CircuitBuilder {
        self.roster = Some(services.into());
        self
    }

    /// Sets the list of node IDs for the members in the circuit
    ///
    /// # Arguments
    ///
    ///  * `members` - List of node IDs
    pub fn with_members(mut self, members: &[String]) -> CircuitBuilder {
        self.members = Some(members.into());
        self
    }

    /// Sets the authorization type
    ///
    /// # Arguments
    ///
    ///  * `authorization_type` - The authorization type for the circuit
    pub fn with_authorization_type(
        mut self,
        authorization_type: &AuthorizationType,
    ) -> CircuitBuilder {
        self.authorization_type = Some(authorization_type.clone());
        self
    }

    /// Sets the persistence type
    ///
    /// # Arguments
    ///
    ///  * `persistence` - The persistence type for the circuit
    pub fn with_persistence(mut self, persistence: &PersistenceType) -> CircuitBuilder {
        self.persistence = Some(persistence.clone());
        self
    }

    /// Sets the durabilitye type
    ///
    /// # Arguments
    ///
    ///  * `durability` - The durability type for the circuit
    pub fn with_durability(mut self, durability: &DurabilityType) -> CircuitBuilder {
        self.durability = Some(durability.clone());
        self
    }

    /// Sets the routing type
    ///
    /// # Arguments
    ///
    ///  * `route_type` - The routing type for the circuit
    pub fn with_routes(mut self, route_type: &RouteType) -> CircuitBuilder {
        self.routes = Some(route_type.clone());
        self
    }

    /// Sets the circuit management type
    ///
    /// # Arguments
    ///
    ///  * `circuit_management_type` - The circuit management type for a circuit
    pub fn with_circuit_management_type(mut self, circuit_management_type: &str) -> CircuitBuilder {
        self.circuit_management_type = Some(circuit_management_type.into());
        self
    }

    /// Builds a `Circuit`
    ///
    /// Returns an error if the circuit ID, roster, members or circuit management
    /// type are not set.
    pub fn build(self) -> Result<Circuit, InvalidStateError> {
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

        members.sort();

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

        let create_circuit_message = Circuit {
            id: circuit_id,
            roster,
            members,
            authorization_type,
            persistence,
            durability,
            routes,
            circuit_management_type,
        };

        Ok(create_circuit_message)
    }
}

impl From<ProposedCircuit> for Circuit {
    fn from(circuit: ProposedCircuit) -> Self {
        Circuit {
            id: circuit.circuit_id().into(),
            roster: circuit.roster().iter().map(Service::from).collect(),
            members: circuit
                .members()
                .iter()
                .map(|node| node.node_id().to_string())
                .collect(),
            authorization_type: circuit.authorization_type().clone(),
            persistence: circuit.persistence().clone(),
            durability: circuit.durability().clone(),
            routes: circuit.routes().clone(),
            circuit_management_type: circuit.circuit_management_type().into(),
        }
    }
}
