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

//! Structs for building services

use crate::admin::messages::{is_valid_circuit_id, is_valid_service_id};

use super::error::BuilderError;
use super::{
    AuthorizationType, CircuitProposal, DurabilityType, PersistenceType, ProposalType,
    ProposedCircuit, ProposedNode, ProposedService, RouteType, VoteRecord,
};

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

    /// Sets the authorization type
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
    pub fn build(self) -> Result<ProposedCircuit, BuilderError> {
        let circuit_id = match self.circuit_id {
            Some(circuit_id) if is_valid_circuit_id(&circuit_id) => circuit_id,
            Some(circuit_id) => {
                return Err(BuilderError::InvalidField(format!(
                    "circuit_id is invalid ({}): must be an 11 character string composed of two, \
                     5 character base62 strings joined with a '-' (example: abcDE-F0123)",
                    circuit_id,
                )))
            }
            None => return Err(BuilderError::MissingField("circuit_id".to_string())),
        };

        let roster = self
            .roster
            .ok_or_else(|| BuilderError::MissingField("roster".to_string()))?;

        let members = self
            .members
            .ok_or_else(|| BuilderError::MissingField("members".to_string()))?;

        let authorization_type = self
            .authorization_type
            .unwrap_or_else(|| AuthorizationType::Trust);

        let persistence = self.persistence.unwrap_or_else(PersistenceType::default);

        let durability = self
            .durability
            .unwrap_or_else(|| DurabilityType::NoDurability);

        let routes = self.routes.unwrap_or_else(RouteType::default);

        let circuit_management_type = self
            .circuit_management_type
            .ok_or_else(|| BuilderError::MissingField("circuit_management_type".to_string()))?;

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

/// Builder to be used to build a `CircuitProposal`
#[derive(Clone, Default)]
pub struct CircuitProposalBuilder {
    proposal_type: Option<ProposalType>,
    circuit_id: Option<String>,
    circuit_hash: Option<String>,
    circuit: Option<ProposedCircuit>,
    votes: Option<Vec<VoteRecord>>,
    requester: Option<Vec<u8>>,
    requester_node_id: Option<String>,
}

impl CircuitProposalBuilder {
    /// Creates a new circuit proposal builder
    pub fn new() -> Self {
        CircuitProposalBuilder::default()
    }

    /// Returns the proposal type of the builder
    pub fn proposal_type(&self) -> Option<ProposalType> {
        self.proposal_type.clone()
    }

    /// Returns the circuit ID
    pub fn circuit_id(&self) -> Option<String> {
        self.circuit_id.clone()
    }

    /// Returns the hash of the proposed circuit
    pub fn circuit_hash(&self) -> Option<String> {
        self.circuit_hash.clone()
    }

    /// Returns the circuit being proposed
    pub fn circuit(&self) -> Option<ProposedCircuit> {
        self.circuit.clone()
    }

    /// Returns the list of current votes
    pub fn votes(&self) -> Option<Vec<VoteRecord>> {
        self.votes.clone()
    }

    /// Returns the public key of the original request of the proposal
    pub fn requester(&self) -> Option<Vec<u8>> {
        self.requester.clone()
    }

    /// Returns the the ID of the node the requester is permissioned to submit proposals for
    pub fn requester_node_id(&self) -> Option<String> {
        self.requester_node_id.clone()
    }

    /// Set the proposal type of the circuit proposal
    ///
    /// # Arguments
    ///
    ///  * `proposal_type` - The type of proposal being built
    pub fn with_proposal_type(mut self, proposal_type: &ProposalType) -> CircuitProposalBuilder {
        self.proposal_type = Some(proposal_type.clone());
        self
    }

    /// Set the circuit ID for the circuit the proposal is for
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique circuit ID for the proposed circuit
    pub fn with_circuit_id(mut self, circuit_id: &str) -> CircuitProposalBuilder {
        self.circuit_id = Some(circuit_id.to_string());
        self
    }

    /// Set the hash the proposed circuit the proposal is for. This will be used to validate votes
    /// are for the correct proposal
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique circuit ID for the proposed circuit
    pub fn with_circuit_hash(mut self, circuit_hash: &str) -> CircuitProposalBuilder {
        self.circuit_hash = Some(circuit_hash.to_string());
        self
    }

    /// Sets the proposed circuit
    ///
    /// # Arguments
    ///
    ///  * `circuit` - The circuit that is being proposed
    pub fn with_circuit(mut self, circuit: &ProposedCircuit) -> CircuitProposalBuilder {
        self.circuit = Some(circuit.clone());
        self
    }

    /// Sets the list of existing vote records
    ///
    /// # Arguments
    ///
    ///  * `votes` - A list of vote records
    pub fn with_votes(mut self, votes: &[VoteRecord]) -> CircuitProposalBuilder {
        self.votes = Some(votes.to_vec());
        self
    }

    /// Sets the public key for the requester of the proposal
    ///
    /// # Arguments
    ///
    ///  * `requester` - The public key of the requester
    pub fn with_requester(mut self, requester: &[u8]) -> CircuitProposalBuilder {
        self.requester = Some(requester.to_vec());
        self
    }

    /// Sets the requester node ID
    ///
    /// # Arguments
    ///
    ///  * `requester_node_od` - The node ID of the node the requester has permissions to submit
    ///       proposals for
    pub fn with_requester_node_id(mut self, requester_node_id: &str) -> CircuitProposalBuilder {
        self.requester_node_id = Some(requester_node_id.to_string());
        self
    }

    /// Builds a `CircuitProposal`
    ///
    /// Returns an error if the circuit ID, circuit, circuit hash, requester, or requester node id
    /// is not set.
    pub fn build(self) -> Result<CircuitProposal, BuilderError> {
        let circuit_id = match self.circuit_id {
            Some(circuit_id) if is_valid_circuit_id(&circuit_id) => circuit_id,
            Some(circuit_id) => {
                return Err(BuilderError::InvalidField(format!(
                    "circuit_id is invalid ({}): must be an 11 character string composed of two, \
                     5 character base62 strings joined with a '-' (example: abcDE-F0123)",
                    circuit_id,
                )))
            }
            None => return Err(BuilderError::MissingField("circuit_id".to_string())),
        };

        let proposal_type = self
            .proposal_type
            .ok_or_else(|| BuilderError::MissingField("proposal_type".to_string()))?;

        let circuit_hash = self
            .circuit_hash
            .ok_or_else(|| BuilderError::MissingField("circuit_hash".to_string()))?;

        let circuit = self
            .circuit
            .ok_or_else(|| BuilderError::MissingField("circuit".to_string()))?;

        let votes = self.votes.unwrap_or_default();

        let requester = self
            .requester
            .ok_or_else(|| BuilderError::MissingField("requester".to_string()))?;

        let requester_node_id = self
            .requester_node_id
            .ok_or_else(|| BuilderError::MissingField("requester node id".to_string()))?;

        Ok(CircuitProposal {
            proposal_type,
            circuit_id,
            circuit_hash,
            circuit,
            votes,
            requester,
            requester_node_id,
        })
    }
}

/// Builder for creating a `ProposedNode`
#[derive(Default, Clone)]
pub struct ProposedNodeBuilder {
    node_id: Option<String>,
    endpoints: Option<Vec<String>>,
}

impl ProposedNodeBuilder {
    /// Creates a `ProposedNodeBuider`
    pub fn new() -> Self {
        ProposedNodeBuilder::default()
    }

    /// Returns the unique node ID
    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    /// Returns the list of endpoints for the node
    pub fn endpoints(&self) -> Option<Vec<String>> {
        self.endpoints.clone()
    }

    /// Sets the node ID
    ///
    /// # Arguments
    ///
    ///  * `node_id` - The unique node ID for node
    pub fn with_node_id(mut self, node_id: &str) -> ProposedNodeBuilder {
        self.node_id = Some(node_id.into());
        self
    }

    /// Sets the endpoints
    ///
    /// # Arguments
    ///
    ///  * `endpoints` - The list of endpoints for the node
    pub fn with_endpoints(mut self, endpoints: &[String]) -> ProposedNodeBuilder {
        self.endpoints = Some(endpoints.into());
        self
    }

    /// Builds the `ProposedNode`
    ///
    /// Returns an error if the node ID or endpoints are not set
    pub fn build(self) -> Result<ProposedNode, BuilderError> {
        let node_id = self
            .node_id
            .ok_or_else(|| BuilderError::MissingField("node_id".to_string()))?;

        let endpoints = self
            .endpoints
            .ok_or_else(|| BuilderError::MissingField("endpoints".to_string()))?;

        let node = ProposedNode { node_id, endpoints };

        Ok(node)
    }
}

/// Builder for creating a `ProposedService`
#[derive(Default, Clone)]
pub struct ProposedServiceBuilder {
    service_id: Option<String>,
    service_type: Option<String>,
    allowed_nodes: Option<Vec<String>>,
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

    /// Returns the list of allowed nodes the service can connect to
    pub fn allowed_nodes(&self) -> Option<Vec<String>> {
        self.allowed_nodes.clone()
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

    /// Sets the allowed nodes
    ///
    /// # Arguments
    ///
    ///  * `allowed_nodes` - A list of node IDs the service can connect to
    pub fn with_allowed_nodes(mut self, allowed_nodes: &[String]) -> ProposedServiceBuilder {
        self.allowed_nodes = Some(allowed_nodes.into());
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
    pub fn build(self) -> Result<ProposedService, BuilderError> {
        let service_id = match self.service_id {
            Some(service_id) if is_valid_service_id(&service_id) => service_id,
            Some(service_id) => {
                return Err(BuilderError::InvalidField(format!(
                    "service_id is invalid ({}): must be a 4 character base62 string",
                    service_id,
                )))
            }
            None => return Err(BuilderError::MissingField("service_id".to_string())),
        };

        let service_type = self
            .service_type
            .ok_or_else(|| BuilderError::MissingField("service_type".to_string()))?;

        let allowed_nodes = self
            .allowed_nodes
            .ok_or_else(|| BuilderError::MissingField("allowed_nodes".to_string()))?;

        let arguments = self.arguments.unwrap_or_default();

        let service = ProposedService {
            service_id,
            service_type,
            allowed_nodes,
            arguments,
        };

        Ok(service)
    }
}
