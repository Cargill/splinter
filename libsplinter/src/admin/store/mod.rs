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

//! Defines an API for managing the API for writing and reading circuit state and pending circuit
//! proposals. The provided `AdminServiceStore` trait makes no assumptions about the storage
//! backend.
//!
//! The public interface includes the trait [`AdminServiceStore`] and structs for
//! [`Circuit`], [`ProposedCircuit`], [`CircuitNode`], [`ProposedNode`], [`Service`],
//! [`ProposedService`], and [`CircuitProposal`]. A YAML backed [`YamlAdminServiceStore`] is
//! also available.
//!
//! Builders are also provided. The structs are [`CircuitBuilder`], [`ProposedCircuitBuilder`],
//! [`CircuitNodeBuilder`], [`ProposedNodeBuilder`], [`ServiceBuilder`],
//! [`ProposedServiceBuilder`], and [`CircuitProposalBuilder`].
//!
//! [`AdminServiceStore`]: trait.AdminServiceStore.html
//! [`Circuit`]: struct.Circuit.html
//! [`ProposedCircuit`]: struct.ProposedCircuit.html
//! [`CircuitNode`]: struct.CircuitNode.html
//! [`ProposedNode`]: struct.ProposedNode.html
//! [`Service`]: struct.Service.html
//! [`ProposedService`]: struct.ProposedService.html
//! [`CircuitProposal`]: struct.CircuitProposal.html
//! [`YamlAdminServiceStore`]: yaml/struct.YamlAdminServiceStore.html
//!
//! [`CircuitBuilder`]: struct.CircuitBuilder.html
//! [`ProposedCircuitBuilder`]: struct.ProposedCircuitBuilder.html
//! [`CircuitNodeBuilder`]: struct.CircuitNodeBuilder.html
//! [`ProposedNodeBuilder`]: struct.ProposedNodeBuilder.html
//! [`ServiceBuilder`]: struct.ServiceBuilder.html
//! [`ProposedServiceBuilder`]: struct.ProposedServiceBuilder.html
//! [`CircuitProposalBuilder`]: struct.CircuitProposalBuilder.html

mod builders;
pub mod error;

use std::cmp::Ordering;
use std::fmt;

use crate::hex::{as_hex, deserialize_hex};

pub use self::builders::{
    CircuitBuilder, CircuitNodeBuilder, CircuitProposalBuilder, ProposedCircuitBuilder,
    ProposedNodeBuilder, ProposedServiceBuilder, ServiceBuilder,
};
use self::error::AdminServiceStoreError;

/// Native representation of a circuit in state
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Circuit {
    id: String,
    roster: Vec<Service>,
    members: Vec<String>,
    auth: AuthorizationType,
    persistence: PersistenceType,
    durability: DurabilityType,
    routes: RouteType,
    circuit_management_type: String,
}

/// Native representation of a circuit that is being proposed in a proposal
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProposedCircuit {
    circuit_id: String,
    roster: Vec<ProposedService>,
    members: Vec<ProposedNode>,
    authorization_type: AuthorizationType,
    persistence: PersistenceType,
    durability: DurabilityType,
    routes: RouteType,
    circuit_management_type: String,
    #[serde(serialize_with = "as_hex")]
    #[serde(deserialize_with = "deserialize_hex")]
    #[serde(default)]
    application_metadata: Vec<u8>,
    comments: String,
}

/// Native representation of a circuit proposal
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CircuitProposal {
    pub proposal_type: ProposalType,
    pub circuit_id: String,
    pub circuit_hash: String,
    pub circuit: ProposedCircuit,
    pub votes: Vec<VoteRecord>,
    #[serde(serialize_with = "as_hex")]
    #[serde(deserialize_with = "deserialize_hex")]
    pub requester: Vec<u8>,
    pub requester_node_id: String,
}

impl CircuitProposal {
    /// Adds a vote record to a pending circuit proposal
    pub fn add_vote(&mut self, vote: VoteRecord) {
        self.votes.push(vote);
    }
}

/// Native representation of a vote record for a proposal
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct VoteRecord {
    pub public_key: Vec<u8>,
    pub vote: Vote,
    pub voter_node_id: String,
}

/// Represents a vote, either accept or reject, for a circuit proposal
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Vote {
    Accept,
    Reject,
}

/// Represents the of  type change the circuit proposal is for
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ProposalType {
    Create,
    UpdateRoster,
    AddNode,
    RemoveNode,
    Destroy,
}

/// What type of authorization the circuit requires
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum AuthorizationType {
    Trust,
}

/// A circuits message persistence strategy
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PersistenceType {
    Any,
}

impl Default for PersistenceType {
    fn default() -> Self {
        PersistenceType::Any
    }
}

/// A circuits durability requirement
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum DurabilityType {
    NoDurability,
}

/// How messages are expected to be routed across a circuit
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum RouteType {
    Any,
}

impl Default for RouteType {
    fn default() -> Self {
        RouteType::Any
    }
}

/// Native representation of a node included in circuit
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CircuitNode {
    id: String,
    endpoints: Vec<String>,
}

/// Native representation of a node in a proposed circuit
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProposedNode {
    node_id: String,
    endpoints: Vec<String>,
}

/// Native representation of a service that is a part of circuit
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Service {
    service_id: String,
    service_type: String,
    allowed_nodes: Vec<String>,
    arguments: Vec<(String, String)>,
}

/// Native representation of a service that is a part of a proposed circuit
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProposedService {
    service_id: String,
    service_type: String,
    allowed_nodes: Vec<String>,
    arguments: Vec<(String, String)>,
}

/// The unique ID of service made up of a circuit ID and the individual service ID.
/// A service ID is only required to be unique from within a circuit.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ServiceId {
    circuit_id: String,
    service_id: String,
}

impl ServiceId {
    /// Create a new service ID
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - the ID of the circuit the service belongs to
    /// * `service_id` - the individual ID for the service
    pub fn new(circuit_id: String, service_id: String) -> Self {
        ServiceId {
            circuit_id,
            service_id,
        }
    }

    /// Returns the circuit ID
    pub fn circuit(&self) -> &str {
        &self.circuit_id
    }

    /// Returns the service ID
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Decompose this ServiceId into a tuple of (<circuit ID>, <service ID>).
    pub fn into_parts(self) -> (String, String) {
        (self.circuit_id, self.service_id)
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.circuit_id, self.service_id)
    }
}

impl Eq for ServiceId {}

impl Ord for ServiceId {
    fn cmp(&self, other: &Self) -> Ordering {
        let compare = self.circuit_id.cmp(&other.circuit_id);
        if compare == Ordering::Equal {
            self.service_id.cmp(&other.service_id)
        } else {
            compare
        }
    }
}

impl PartialOrd for ServiceId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Predicate for filtering the lists of circuits and circuit proposals
pub enum CircuitPredicate {
    ManagmentTypeEq(String),
    MembersInclude(Vec<String>),
}

impl CircuitPredicate {
    /// Apply this predicate against a given circuit
    pub fn apply_to_circuit(&self, circuit: &Circuit) -> bool {
        match self {
            CircuitPredicate::ManagmentTypeEq(man_type) => {
                &circuit.circuit_management_type == man_type
            }
            CircuitPredicate::MembersInclude(nodes) => {
                for node_id in nodes.iter() {
                    if !circuit.members.contains(node_id) {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// Apply this predicate against a given circuit proposal
    pub fn apply_to_proposals(&self, proposal: &CircuitProposal) -> bool {
        match self {
            CircuitPredicate::ManagmentTypeEq(man_type) => {
                &proposal.circuit.circuit_management_type == man_type
            }
            CircuitPredicate::MembersInclude(nodes) => {
                for node_id in nodes {
                    if proposal
                        .circuit
                        .members
                        .iter()
                        .find(|node| node_id == &node.node_id)
                        .is_none()
                    {
                        return false;
                    }
                }
                true
            }
        }
    }
}

/// Defines methods for CRUD operations and fetching and listing circuits, proposals, nodes and
/// services without defining a storage strategy
pub trait AdminServiceStore: Send + Sync {
    /// Adds a circuit proposal to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal` - The proposal to be added
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID already exists
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError>;

    /// Updates a circuit proposal in the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal` - The proposal with the updated information
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID does not exist
    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError>;

    /// Removes a circuit proposal from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal_id` - The unique ID of the circuit proposal to be removed
    ///
    ///  Returns an error if a `CircuitProposal` with specified ID does not exist
    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError>;

    /// Fetches a circuit proposal from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal_id` - The unique ID of the circuit proposal to be returned
    fn fetch_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError>;

    /// List circuit proposals from the underlying storage
    ///
    /// The proposals returned can be filtered by provided `CircuitPredicate`. This enables
    /// filtering by management type and members.
    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError>;

    /// Adds a circuit to the underlying storage. Also includes the associated Services and
    /// Nodes
    ///
    /// # Arguments
    ///
    ///  * `circuit` - The circuit to be added to state
    ///  * `nodes` - A list of nodes that represent the circuit's members
    ///
    ///  Returns an error if a `Circuit` with the same ID already exists
    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError>;

    /// Updates a circuit in the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit` - The circuit with the updated information
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID does not exist
    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError>;

    /// Removes a circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit to be removed
    ///
    ///  Returns an error if a `Circuit` with the specified ID does not exist
    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError>;

    /// Fetches a circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit to be returned
    fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError>;

    /// List all circuits from the underlying storage
    ///
    /// The proposals returned can be filtered by provided `CircuitPredicate`. This enables
    /// filtering by management type and members.
    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError>;

    /// Adds a circuit to the underlying storage based on the proposal that is already in state.
    /// Also includes the associated Services and Nodes. The associated circuit proposal for
    /// the circuit ID is also removed
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The ID of the circuit proposal that should be converted to a circuit
    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError>;

    /// Fetches a node from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `node_id` - The unique ID of the node to be returned
    fn fetch_node(&self, node_id: &str) -> Result<Option<CircuitNode>, AdminServiceStoreError>;

    /// List all nodes from the underlying storage
    fn list_nodes(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitNode>>, AdminServiceStoreError>;

    /// Fetches a service from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The `ServiceId` of a service made up of the circuit ID and service ID
    fn fetch_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError>;

    /// List all services in a specific circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit the services belong to
    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError>;
}
