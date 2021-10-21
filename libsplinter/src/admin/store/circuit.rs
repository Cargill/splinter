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

//! Structs for building circuits

use std::convert::TryFrom;

use crate::admin::messages::{self, is_valid_circuit_id};
use crate::circuit::routing;
use crate::error::InvalidStateError;
use crate::protos::admin;

use super::{
    CircuitNode, ProposedCircuit, ProposedNode, Service, ServiceBuilder, UNSET_CIRCUIT_VERSION,
};

/// Native representation of a circuit in state
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circuit {
    id: String,
    roster: Vec<Service>,
    members: Vec<CircuitNode>,
    authorization_type: AuthorizationType,
    persistence: PersistenceType,
    durability: DurabilityType,
    routes: RouteType,
    circuit_management_type: String,
    display_name: Option<String>,
    circuit_version: i32,
    circuit_status: CircuitStatus,
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
    pub fn members(&self) -> &[CircuitNode] {
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

    /// Returns the display name for the circuit
    pub fn display_name(&self) -> &Option<String> {
        &self.display_name
    }

    /// Returns the circuit version for the circuit
    pub fn circuit_version(&self) -> i32 {
        self.circuit_version
    }

    /// Returns the status of the circuit
    pub fn circuit_status(&self) -> &CircuitStatus {
        &self.circuit_status
    }
}

impl TryFrom<&admin::Circuit> for Circuit {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit) -> Result<Self, Self::Error> {
        let roster = proto
            .get_roster()
            .iter()
            .map(|service| {
                ServiceBuilder::new()
                    .with_service_id(service.get_service_id())
                    .with_service_type(service.get_service_type())
                    .with_node_id(service.get_allowed_nodes().get(0).ok_or_else(|| {
                        InvalidStateError::with_message("No node ID was provided".to_string())
                    })?)
                    .with_arguments(
                        &service
                            .get_arguments()
                            .iter()
                            .map(|arg| (arg.get_key().to_string(), arg.get_value().to_string()))
                            .collect::<Vec<(String, String)>>(),
                    )
                    .build()
            })
            .collect::<Result<Vec<Service>, InvalidStateError>>()?;
        let members = proto
            .get_members()
            .iter()
            .map(|node| {
                let propose_node = ProposedNode::from_proto(node.clone());
                CircuitNode::from(propose_node)
            })
            .collect::<Vec<CircuitNode>>();
        let mut builder = CircuitBuilder::new()
            .with_circuit_id(proto.get_circuit_id())
            .with_roster(&roster)
            .with_members(&members)
            .with_authorization_type(&AuthorizationType::try_from(
                &proto.get_authorization_type(),
            )?)
            .with_persistence(&PersistenceType::try_from(&proto.get_persistence())?)
            .with_durability(&DurabilityType::try_from(&proto.get_durability())?)
            .with_routes(&RouteType::try_from(&proto.get_routes())?)
            .with_circuit_management_type(proto.get_circuit_management_type())
            .with_circuit_version(proto.get_circuit_version())
            .with_circuit_status(&CircuitStatus::try_from(&proto.get_circuit_status())?);
        if !proto.get_display_name().is_empty() {
            builder = builder.with_display_name(proto.get_display_name());
        }

        builder.build()
    }
}

/// What type of authorization the circuit requires
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizationType {
    Trust,
    Challenge,
}

impl From<&messages::AuthorizationType> for AuthorizationType {
    fn from(message_enum: &messages::AuthorizationType) -> Self {
        match *message_enum {
            messages::AuthorizationType::Trust => AuthorizationType::Trust,
            messages::AuthorizationType::Challenge => AuthorizationType::Challenge,
        }
    }
}

impl TryFrom<&admin::Circuit_AuthorizationType> for AuthorizationType {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit_AuthorizationType) -> Result<Self, Self::Error> {
        match *proto {
            admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION => Ok(AuthorizationType::Trust),
            admin::Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION => {
                Ok(AuthorizationType::Challenge)
            }
            _ => Err(InvalidStateError::with_message(
                "AuthorizationType is unsupported".to_string(),
            )),
        }
    }
}

impl From<&AuthorizationType> for admin::Circuit_AuthorizationType {
    fn from(auth: &AuthorizationType) -> Self {
        match *auth {
            AuthorizationType::Trust => admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION,
            AuthorizationType::Challenge => {
                admin::Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION
            }
        }
    }
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

impl From<&messages::PersistenceType> for PersistenceType {
    fn from(message_enum: &messages::PersistenceType) -> Self {
        match *message_enum {
            messages::PersistenceType::Any => PersistenceType::Any,
        }
    }
}

impl TryFrom<&admin::Circuit_PersistenceType> for PersistenceType {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit_PersistenceType) -> Result<Self, Self::Error> {
        match *proto {
            admin::Circuit_PersistenceType::ANY_PERSISTENCE => Ok(PersistenceType::Any),
            admin::Circuit_PersistenceType::UNSET_PERSISTENCE_TYPE => Err(
                InvalidStateError::with_message("PersistenceType is unset".to_string()),
            ),
        }
    }
}

impl From<&PersistenceType> for admin::Circuit_PersistenceType {
    fn from(persistence: &PersistenceType) -> Self {
        match *persistence {
            PersistenceType::Any => admin::Circuit_PersistenceType::ANY_PERSISTENCE,
        }
    }
}

/// A circuits durability requirement
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurabilityType {
    NoDurability,
}

impl From<&messages::DurabilityType> for DurabilityType {
    fn from(message_enum: &messages::DurabilityType) -> Self {
        match *message_enum {
            messages::DurabilityType::NoDurability => DurabilityType::NoDurability,
        }
    }
}

impl TryFrom<&admin::Circuit_DurabilityType> for DurabilityType {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit_DurabilityType) -> Result<Self, Self::Error> {
        match *proto {
            admin::Circuit_DurabilityType::NO_DURABILITY => Ok(DurabilityType::NoDurability),
            admin::Circuit_DurabilityType::UNSET_DURABILITY_TYPE => Err(
                InvalidStateError::with_message("DurabilityType is unset".to_string()),
            ),
        }
    }
}

impl From<&DurabilityType> for admin::Circuit_DurabilityType {
    fn from(durability: &DurabilityType) -> Self {
        match *durability {
            DurabilityType::NoDurability => admin::Circuit_DurabilityType::NO_DURABILITY,
        }
    }
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

impl From<&messages::RouteType> for RouteType {
    fn from(message_enum: &messages::RouteType) -> Self {
        match *message_enum {
            messages::RouteType::Any => RouteType::Any,
        }
    }
}

impl TryFrom<&admin::Circuit_RouteType> for RouteType {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit_RouteType) -> Result<Self, Self::Error> {
        match *proto {
            admin::Circuit_RouteType::ANY_ROUTE => Ok(RouteType::Any),
            admin::Circuit_RouteType::UNSET_ROUTE_TYPE => Err(InvalidStateError::with_message(
                "RouteType is unset".to_string(),
            )),
        }
    }
}

impl From<&RouteType> for admin::Circuit_RouteType {
    fn from(route: &RouteType) -> Self {
        match *route {
            RouteType::Any => admin::Circuit_RouteType::ANY_ROUTE,
        }
    }
}

/// Status of the circuit
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitStatus {
    Active,
    Disbanded,
    Abandoned,
}

impl Default for CircuitStatus {
    fn default() -> Self {
        CircuitStatus::Active
    }
}

impl From<&messages::CircuitStatus> for CircuitStatus {
    fn from(message_enum: &messages::CircuitStatus) -> Self {
        match *message_enum {
            messages::CircuitStatus::Active => CircuitStatus::Active,
            messages::CircuitStatus::Disbanded => CircuitStatus::Disbanded,
            messages::CircuitStatus::Abandoned => CircuitStatus::Abandoned,
        }
    }
}

impl TryFrom<&admin::Circuit_CircuitStatus> for CircuitStatus {
    type Error = InvalidStateError;

    fn try_from(proto: &admin::Circuit_CircuitStatus) -> Result<Self, Self::Error> {
        match *proto {
            admin::Circuit_CircuitStatus::ACTIVE => Ok(CircuitStatus::Active),
            admin::Circuit_CircuitStatus::DISBANDED => Ok(CircuitStatus::Disbanded),
            admin::Circuit_CircuitStatus::ABANDONED => Ok(CircuitStatus::Abandoned),
            admin::Circuit_CircuitStatus::UNSET_CIRCUIT_STATUS => {
                debug!("Defaulting `UNSET_CIRCUIT_STATUS` of proposed circuit to `Active`");
                Ok(CircuitStatus::Active)
            }
        }
    }
}

impl From<&CircuitStatus> for admin::Circuit_CircuitStatus {
    fn from(status: &CircuitStatus) -> Self {
        match *status {
            CircuitStatus::Active => admin::Circuit_CircuitStatus::ACTIVE,
            CircuitStatus::Disbanded => admin::Circuit_CircuitStatus::DISBANDED,
            CircuitStatus::Abandoned => admin::Circuit_CircuitStatus::ABANDONED,
        }
    }
}

/// Builder to be used to build a `Circuit`
#[derive(Default, Clone)]
pub struct CircuitBuilder {
    circuit_id: Option<String>,
    roster: Option<Vec<Service>>,
    members: Option<Vec<CircuitNode>>,
    authorization_type: Option<AuthorizationType>,
    persistence: Option<PersistenceType>,
    durability: Option<DurabilityType>,
    routes: Option<RouteType>,
    circuit_management_type: Option<String>,
    display_name: Option<String>,
    circuit_version: Option<i32>,
    circuit_status: Option<CircuitStatus>,
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
    pub fn members(&self) -> Option<Vec<CircuitNode>> {
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

    /// Returns the display_name in the builder
    pub fn display_name(&self) -> Option<String> {
        self.display_name.clone()
    }

    /// Returns the circuit version in the builder
    pub fn circuit_version(&self) -> Option<i32> {
        self.circuit_version
    }

    /// Returns the circuit status in the builder
    pub fn circuit_status(&self) -> Option<CircuitStatus> {
        self.circuit_status.clone()
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
    ///  * `members` - List of CircuitNodes
    pub fn with_members(mut self, members: &[CircuitNode]) -> CircuitBuilder {
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

    /// Sets the display name for the circuit
    ///
    /// # Arguments
    ///
    ///  * `display_name` - The human readable display name for the circuit
    pub fn with_display_name(mut self, display_name: &str) -> CircuitBuilder {
        self.display_name = Some(display_name.into());
        self
    }

    /// Sets the circuit version for the circuit
    ///
    /// # Arguments
    ///
    ///  * `circuit_version` - The protocol version the circuit must implement
    ///
    /// If this is not set, the circuit version is assumed to be 1.
    pub fn with_circuit_version(mut self, circuit_version: i32) -> CircuitBuilder {
        self.circuit_version = Some(circuit_version);
        self
    }

    /// Sets the status for the circuit
    ///
    /// # Arguments
    ///
    ///  * `circuit_status` - The status for the circuit
    pub fn with_circuit_status(mut self, circuit_status: &CircuitStatus) -> CircuitBuilder {
        self.circuit_status = Some(circuit_status.clone());
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

        let roster = self.roster.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `roster`".to_string())
        })?;

        let members = self.members.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `members`".to_string())
        })?;

        let authorization_type = self.authorization_type.unwrap_or(AuthorizationType::Trust);

        let persistence = self.persistence.unwrap_or_default();

        let durability = self.durability.unwrap_or(DurabilityType::NoDurability);

        let routes = self.routes.unwrap_or_default();

        let circuit_management_type = self.circuit_management_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `circuit_management_type`".to_string(),
            )
        })?;

        let display_name = self.display_name;

        let circuit_version = self.circuit_version.unwrap_or(UNSET_CIRCUIT_VERSION);

        let circuit_status = self.circuit_status.unwrap_or_default();

        let circuit = Circuit {
            id: circuit_id,
            roster,
            members,
            authorization_type,
            persistence,
            durability,
            routes,
            circuit_management_type,
            display_name,
            circuit_version,
            circuit_status,
        };

        Ok(circuit)
    }
}

impl From<ProposedCircuit> for Circuit {
    fn from(circuit: ProposedCircuit) -> Self {
        Circuit {
            id: circuit.circuit_id().into(),
            roster: circuit.roster().iter().map(Service::from).collect(),
            members: circuit.members().iter().map(CircuitNode::from).collect(),
            authorization_type: circuit.authorization_type().clone(),
            persistence: circuit.persistence().clone(),
            durability: circuit.durability().clone(),
            routes: circuit.routes().clone(),
            circuit_management_type: circuit.circuit_management_type().into(),
            display_name: circuit.display_name().clone(),
            circuit_version: circuit.circuit_version(),
            circuit_status: circuit.circuit_status().clone(),
        }
    }
}

impl From<&AuthorizationType> for routing::AuthorizationType {
    fn from(auth_type: &AuthorizationType) -> Self {
        match auth_type {
            AuthorizationType::Trust => routing::AuthorizationType::Trust,
            AuthorizationType::Challenge => routing::AuthorizationType::Challenge,
        }
    }
}
