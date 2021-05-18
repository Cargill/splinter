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

//! Provides functionality for [`CircuitBuilder`]s

use splinter::admin::messages::CreateCircuit;
use splinter::admin::messages::{
    AuthorizationType, CircuitStatus, CreateCircuitBuilder, DurabilityType, PersistenceType,
    RouteType, SplinterNode, SplinterNodeBuilder, SplinterService, SplinterServiceBuilder,
};

use super::{CircuitBuildError, NodeCollection};
use crate::admin::payload::{complete_create_payload, make_circuit_proposal_vote_payload};
use crate::framework::network::Network;
use splinter::admin::client::event::{AdminServiceEvent, AdminServiceEventClient, EventType};
use splinter::error::{InternalError, InvalidArgumentError};
use splinterd::node::Node;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

/// `CircuitBuilder` is a framework for quickly building circuits with a variety of service
/// configurations. It provides a high-level API for quickly setting up peered networks,
/// and low-level access for fine-tuning circuit creation details.
pub struct CircuitBuilder<'a, N = Network>
where
    Self: 'a,
    N: NodeCollection,
{
    proposal_builder: CreateCircuitBuilder,
    network: &'a N,
    nodes: Vec<&'a Node>,
    service_id_generator: ServiceIdGenerator,
}

impl<'a, N> CircuitBuilder<'a, N>
where
    Self: 'a,
    N: NodeCollection,
{
    /// Create a new `CircuitBuilder`, where the passed nodes are the node indices from the
    /// associated network
    pub fn new(
        network: &'a N,
        nodes: &[usize],
    ) -> Result<CircuitBuilder<'a, N>, InvalidArgumentError> {
        if nodes.is_empty() {
            return Err(InvalidArgumentError::new(
                "nodes".to_string(),
                "there must be 1 or more nodes in a circuit".to_string(),
            ));
        }

        Ok(CircuitBuilder {
            network,
            proposal_builder: CreateCircuitBuilder::new(),
            nodes: nodes
                .iter()
                .map(|i| network.node(*i))
                .collect::<Result<Vec<&Node>, InvalidArgumentError>>()?,
            service_id_generator: ServiceIdGenerator::new(),
        }
        .with_defaults())
    }

    /// Get the `Network` for this `CircuitBuilder`
    pub fn network(&self) -> &'a N {
        self.network
    }

    /// Get all the the member nodes in this circuit
    pub fn nodes(&self) -> Vec<&'a Node> {
        self.nodes.clone()
    }

    /// Get a generator for service ids
    pub(crate) fn service_id_generator(&mut self) -> &mut ServiceIdGenerator {
        &mut self.service_id_generator
    }

    /// Transform this `CircuitBuilder` into another object, such as a [`ScabbardCircuitBuilderVeil`]
    pub fn veil<T: From<CircuitBuilder<'a, N>>>(self) -> T {
        T::from(self)
    }

    /// Add reasonable defaults to the circuit proposal
    pub fn with_defaults(mut self) -> Self {
        let circuit_id = "abcDE-F0123";
        self.proposal_builder = self
            .proposal_builder
            .with_circuit_id(circuit_id)
            .with_circuit_management_type(&format!("test_circuit_{}", &circuit_id))
            .with_authorization_type(&AuthorizationType::Trust)
            .with_persistence(&PersistenceType::Any)
            .with_durability(&DurabilityType::NoDurability)
            .with_routes(&RouteType::Any)
            .with_application_metadata(b"test_data")
            .with_comments("test circuit")
            .with_display_name("test_circuit")
            .with_circuit_version(2);
        self
    }

    pub fn circuit_id(&self) -> Option<String> {
        self.proposal_builder.circuit_id()
    }

    pub fn roster(&self) -> Option<Vec<SplinterService>> {
        self.proposal_builder.roster()
    }

    pub fn authorization_type(&self) -> Option<AuthorizationType> {
        self.proposal_builder.authorization_type()
    }

    pub fn persistence(&self) -> Option<PersistenceType> {
        self.proposal_builder.persistence()
    }

    pub fn durability(&self) -> Option<DurabilityType> {
        self.proposal_builder.durability()
    }

    pub fn routes(&self) -> Option<RouteType> {
        self.proposal_builder.routes()
    }

    pub fn circuit_management_type(&self) -> Option<String> {
        self.proposal_builder.circuit_management_type()
    }

    pub fn application_metadata(&self) -> Option<Vec<u8>> {
        self.proposal_builder.application_metadata()
    }

    pub fn comments(&self) -> Option<String> {
        self.proposal_builder.comments()
    }

    pub fn display_name(&self) -> Option<String> {
        self.proposal_builder.display_name()
    }

    pub fn circuit_version(&self) -> Option<i32> {
        self.proposal_builder.circuit_version()
    }

    pub fn circuit_status(&self) -> Option<CircuitStatus> {
        self.proposal_builder.circuit_status()
    }

    pub fn with_circuit_id(mut self, circuit_id: &str) -> Self {
        self.proposal_builder = self.proposal_builder.with_circuit_id(circuit_id);
        self
    }

    pub fn with_roster(mut self, services: &[SplinterService]) -> Self {
        self.proposal_builder = self.proposal_builder.with_roster(services);
        self
    }

    pub fn with_authorization_type(mut self, authorization_type: &AuthorizationType) -> Self {
        self.proposal_builder = self
            .proposal_builder
            .with_authorization_type(authorization_type);
        self
    }

    pub fn with_persistence(mut self, persistence: &PersistenceType) -> Self {
        self.proposal_builder = self.proposal_builder.with_persistence(persistence);
        self
    }

    pub fn with_durability(mut self, durability: &DurabilityType) -> Self {
        self.proposal_builder = self.proposal_builder.with_durability(durability);
        self
    }

    pub fn with_routes(mut self, route_type: &RouteType) -> Self {
        self.proposal_builder = self.proposal_builder.with_routes(route_type);
        self
    }

    pub fn with_circuit_management_type(mut self, circuit_management_type: &str) -> Self {
        self.proposal_builder = self
            .proposal_builder
            .with_circuit_management_type(circuit_management_type);
        self
    }

    pub fn with_application_metadata(mut self, application_metadata: &[u8]) -> Self {
        self.proposal_builder = self
            .proposal_builder
            .with_application_metadata(application_metadata);
        self
    }

    pub fn with_comments(mut self, comments: &str) -> Self {
        self.proposal_builder = self.proposal_builder.with_comments(comments);
        self
    }

    pub fn with_display_name(mut self, display_name: &str) -> Self {
        self.proposal_builder = self.proposal_builder.with_display_name(display_name);
        self
    }

    pub fn with_circuit_version(mut self, circuit_version: i32) -> Self {
        self.proposal_builder = self.proposal_builder.with_circuit_version(circuit_version);
        self
    }

    pub fn with_circuit_status(mut self, status: &CircuitStatus) -> Self {
        self.proposal_builder = self.proposal_builder.with_circuit_status(status);
        self
    }

    /// Build the circuit, consuming this `CircuitBuilder` and receiving a [`CircuitData`] back
    pub fn build(self) -> Result<CircuitData<'a>, CircuitBuildError> {
        fn internal_error(e: impl Error + 'static) -> CircuitBuildError {
            CircuitBuildError::Internal(InternalError::from_source(Box::new(e)))
        }

        let nodes = self.nodes();

        let node_first = nodes
            .get(0)
            .ok_or_else(|| InternalError::with_message("unexpected no nodes".to_string()))?;

        let splinter_nodes: Vec<SplinterNode> = nodes
            .iter()
            .map(|node| {
                SplinterNodeBuilder::new()
                    .with_node_id(&node.node_id())
                    .with_endpoints(&node.network_endpoints().to_vec())
                    .build()
                    .map_err(internal_error)
            })
            .collect::<Result<Vec<SplinterNode>, CircuitBuildError>>()?;

        let create_circuit_message = self
            .proposal_builder
            .with_members(&splinter_nodes)
            .build()
            .map_err(internal_error)?;

        let circuit_payload_bytes = complete_create_payload(
            node_first.node_id(),
            &*node_first.admin_signer().clone_box(),
            create_circuit_message
                .clone()
                .into_proto()
                .map_err(internal_error)?,
        )?;

        // Submit the `CircuitManagementPayload` to the first node
        node_first
            .admin_service_client()
            .submit_admin_payload(circuit_payload_bytes)?;

        let node_event_clients = nodes
            .iter()
            .map(|node| {
                node.admin_service_event_client(&create_circuit_message.circuit_management_type)
            })
            .collect::<Result<Vec<Box<dyn AdminServiceEventClient>>, InternalError>>()?;

        // Wait for the proposal event from each node.
        let proposal_event = node_event_clients
            .iter()
            .map(|client| {
                let event = client.next_event().map_err(internal_error)?;

                match event.event_type() {
                    EventType::ProposalSubmitted { .. } => Ok(event),
                    event_type => Err(CircuitBuildError::UnexpectedEvent {
                        expected: "ProposalSubmitted".to_string(),
                        got: event_type.clone(),
                    }),
                }
            })
            .collect::<Result<Vec<AdminServiceEvent>, CircuitBuildError>>()?;

        // Create the `CircuitProposalVote` to be sent to a node
        let proposal = proposal_event
            .get(0)
            .ok_or_else(|| {
                InternalError::with_message("could not get first proposal event".to_string())
            })?
            .proposal();
        for (nodeidx, node) in nodes.iter().enumerate().skip(1) {
            let vote_payload_bytes = make_circuit_proposal_vote_payload(
                proposal.clone(),
                node.node_id(),
                &*node.admin_signer().clone_box(),
                true, // accept
            );
            node.admin_service_client()
                .submit_admin_payload(vote_payload_bytes)?;

            if nodeidx < (nodes.len() - 1) {
                // Wait for ProposalVote
                for client in node_event_clients.iter() {
                    let event = client.next_event().map_err(internal_error)?;

                    match event.event_type() {
                        EventType::ProposalVote { .. } => (),
                        event_type => {
                            return Err(CircuitBuildError::UnexpectedEvent {
                                expected: "ProposalVote".to_string(),
                                got: event_type.clone(),
                            })
                        }
                    }
                }
            }
        }

        // Wait for ProposalAccepted
        for client in node_event_clients.iter() {
            let event = client.next_event().map_err(internal_error)?;

            match event.event_type() {
                EventType::ProposalAccepted { .. } => (),
                event_type => {
                    return Err(CircuitBuildError::UnexpectedEvent {
                        expected: "ProposalAccepted".to_string(),
                        got: event_type.clone(),
                    })
                }
            }
        }

        // Wait for circuit ready.
        for client in node_event_clients.iter() {
            let event = client.next_event().map_err(internal_error)?;

            if event.event_type() != &EventType::CircuitReady {
                return Err(CircuitBuildError::UnexpectedEvent {
                    expected: "CircuitReady".to_string(),
                    got: event.event_type().clone(),
                });
            }
        }

        let node_by_id: HashMap<&str, &Node> =
            nodes.iter().map(|node| (node.node_id(), *node)).collect();

        let data = CircuitData::from_members_and_message(node_by_id, create_circuit_message)
            .map_err(internal_error)?;
        Ok(data)
    }
}

pub struct CircuitService<'a, N = Node> {
    pub id: String,
    pub node: &'a N,
}

/// `CircuitData` contains the data relating to an existing circuit
pub struct CircuitData<'a, N = Node> {
    pub circuit_id: String,
    pub management_type: String,
    pub roster: Vec<CircuitService<'a, N>>,
}

impl<'a, N> CircuitData<'a, N> {
    fn from_members_and_message(
        node_by_id: HashMap<&'a str, &'a N>,
        message: CreateCircuit,
    ) -> Result<Self, InvalidArgumentError> {
        Ok(CircuitData {
            circuit_id: message.circuit_id,
            management_type: message.circuit_management_type,
            roster: message
                .roster
                .iter()
                .map(|service| {
                    let id = get_service_owner(service)?;
                    let node = node_by_id[id];
                    Ok(CircuitService {
                        id: String::from(id),
                        node,
                    })
                })
                .collect::<Result<Vec<CircuitService<'a, N>>, InvalidArgumentError>>()?,
        })
    }
}

fn get_service_owner(service: &SplinterService) -> Result<&str, InvalidArgumentError> {
    let id = service.allowed_nodes.get(0).ok_or_else(|| {
        InvalidArgumentError::new(
            "service".to_string(),
            "there must be allowed nodes".to_string(),
        )
    })?;
    Ok(&id[..])
}

#[derive(Clone)]
/// `ServiceIdGenerator` is a helper utility for generating service ids
pub(crate) struct ServiceIdGenerator {
    indexes: HashMap<&'static str, usize>,
}

impl ServiceIdGenerator {
    pub fn new() -> Self {
        ServiceIdGenerator {
            indexes: HashMap::new(),
        }
    }

    /// Generate the next service id (ex: "sc01")
    pub fn next(&mut self, prefix: &'static str) -> String {
        let idx = self.indexes.entry(prefix).or_insert(1);
        let result = format!("{}{:0>2}", prefix, idx);
        *idx += 1;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    /// A fake [`NodeCollection`]. Significantly speeds up unit tests that do not require access to
    /// actual running [`Node`]s, which requires a comparably expensive network setup.
    pub struct FakeNodeCollection {}

    impl NodeCollection for FakeNodeCollection {
        fn node(&self, _id: usize) -> Result<&Node, InvalidArgumentError> {
            Err(InvalidArgumentError::new(
                "id".to_string(),
                "FakeNodeCollection cannot get nodes".to_string(),
            ))
        }
    }

    #[derive(Debug)]
    enum CircuitTestError {
        Internal(InternalError),
        InvalidArgument(InvalidArgumentError),
    }

    impl fmt::Display for CircuitTestError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                CircuitTestError::Internal(_) => f.write_str("internal error encountered"),
                CircuitTestError::InvalidArgument(_) => f.write_str("invalid argument"),
            }
        }
    }

    impl Error for CircuitTestError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match *self {
                CircuitTestError::Internal(ref e) => Some(e),
                CircuitTestError::InvalidArgument(ref e) => Some(e),
            }
        }
    }

    /// Verify that the `CircuitBuilder` works properly and returns correct `CreateCircuit`
    /// when all fields are set.
    #[test]
    fn circuit_builder_successful() -> Result<(), CircuitTestError> {
        let network = Network::new()
            .add_nodes_with_defaults(2)
            .map_err(CircuitTestError::Internal)?;

        let builder = network
            .circuit_builder(&[0, 1])
            .map_err(CircuitTestError::InvalidArgument)?;

        let builder_node_ids = builder
            .nodes()
            .iter()
            .map(|node| node.node_id())
            .collect::<Vec<&str>>();
        assert_eq!(
            builder_node_ids,
            [
                network.node(0).unwrap().node_id(),
                network.node(1).unwrap().node_id()
            ]
        );

        Ok(())
    }

    /// Verify that the circuit builder changes defaults as expected
    #[test]
    fn circuit_builder_defaults_correct() -> Result<(), CircuitTestError> {
        let network = FakeNodeCollection {};
        let builder = CircuitBuilder {
            network: &network,
            proposal_builder: CreateCircuitBuilder::new(),
            nodes: vec![],
            service_id_generator: ServiceIdGenerator::new(),
        };

        assert_eq!(builder.authorization_type(), None);
        assert_eq!(builder.persistence(), None);
        assert_eq!(builder.durability(), None);
        assert_eq!(builder.routes(), None);
        assert_eq!(builder.application_metadata(), None);
        assert_eq!(builder.comments(), None);
        assert_eq!(builder.display_name(), None);
        assert_eq!(builder.circuit_version(), None);

        let builder = builder.with_defaults();

        assert_eq!(builder.authorization_type(), Some(AuthorizationType::Trust));
        assert_eq!(builder.persistence(), Some(PersistenceType::Any));
        assert_eq!(builder.durability(), Some(DurabilityType::NoDurability));
        assert_eq!(builder.routes(), Some(RouteType::Any));
        assert_eq!(builder.application_metadata(), Some(b"test_data".to_vec()));
        assert_eq!(builder.comments(), Some("test circuit".to_string()));
        assert_eq!(builder.display_name(), Some("test_circuit".to_string()));
        assert_eq!(builder.circuit_version(), Some(2));

        Ok(())
    }

    fn test_service() -> SplinterService {
        SplinterServiceBuilder::new()
            .with_service_type("service_type")
            .with_allowed_nodes(&["node_id".into()])
            .build()
            .expect("failed to build service")
    }

    /// Verify that the circuit builder setters set correctly
    #[test]
    fn circuit_builder_sets_correct() -> Result<(), CircuitTestError> {
        let network = FakeNodeCollection {};
        let builder = CircuitBuilder {
            network: &network,
            proposal_builder: CreateCircuitBuilder::new(),
            nodes: vec![],
            service_id_generator: ServiceIdGenerator::new(),
        };

        assert_eq!(builder.circuit_id(), None);
        assert_eq!(builder.roster(), None);
        assert_eq!(builder.authorization_type(), None);
        assert_eq!(builder.persistence(), None);
        assert_eq!(builder.durability(), None);
        assert_eq!(builder.routes(), None);
        assert_eq!(builder.circuit_management_type(), None);
        assert_eq!(builder.application_metadata(), None);
        assert_eq!(builder.comments(), None);
        assert_eq!(builder.display_name(), None);
        assert_eq!(builder.circuit_version(), None);
        assert_eq!(builder.circuit_status(), None);

        let circuit_id: &str = "asdf";
        let services: &[SplinterService] = &[test_service()];
        let authorization_type: &AuthorizationType = &AuthorizationType::Trust;
        let persistence: &PersistenceType = &PersistenceType::Any;
        let durability: &DurabilityType = &DurabilityType::NoDurability;
        let route_type: &RouteType = &RouteType::Any;
        let circuit_management_type: &str = "test_circuit_asdf";
        let application_metadata: &[u8] = &[10, 1, 2];
        let comments: &str = "comment";
        let display_name: &str = "some display name";
        let circuit_version: i32 = 2;
        let status: &CircuitStatus = &CircuitStatus::Disbanded;

        let builder = builder
            .with_circuit_id(circuit_id)
            .with_roster(services)
            .with_authorization_type(authorization_type)
            .with_persistence(persistence)
            .with_durability(durability)
            .with_routes(route_type)
            .with_circuit_management_type(circuit_management_type)
            .with_application_metadata(application_metadata)
            .with_comments(comments)
            .with_display_name(display_name)
            .with_circuit_version(circuit_version)
            .with_circuit_status(status);

        assert_eq!(builder.circuit_id().as_deref(), Some(circuit_id));
        assert_eq!(builder.roster().as_deref(), Some(services));
        assert_eq!(
            builder.authorization_type().as_ref(),
            Some(authorization_type)
        );
        assert_eq!(builder.persistence().as_ref(), Some(persistence));
        assert_eq!(builder.durability().as_ref(), Some(durability));
        assert_eq!(builder.routes().as_ref(), Some(route_type));
        assert_eq!(
            builder.circuit_management_type().as_deref(),
            Some(circuit_management_type)
        );
        assert_eq!(
            builder.application_metadata().as_deref(),
            Some(application_metadata)
        );
        assert_eq!(builder.comments().as_deref(), Some(comments));
        assert_eq!(builder.display_name().as_deref(), Some(display_name));
        assert_eq!(builder.circuit_version(), Some(circuit_version));
        assert_eq!(builder.circuit_status().as_ref(), Some(status));

        Ok(())
    }

    #[test]
    fn service_id_generator_uniquely_names_services() {
        let mut generator = ServiceIdGenerator::new();
        assert_eq!(generator.next("sc"), "sc01");
        assert_eq!(generator.next("sc"), "sc02");
        assert_eq!(generator.next("sc"), "sc03");
        assert_eq!(generator.next("sw"), "sw01");
        assert_eq!(generator.next("sw"), "sw02");
        assert_eq!(generator.next("sw"), "sw03");
    }

    #[test]
    fn service_owner_corect() {
        let owner = "node01".to_string();
        let service = SplinterService {
            service_id: "sc01".to_string(),
            service_type: "scabbard".to_string(),
            allowed_nodes: vec![owner.clone()],
            arguments: vec![],
        };
        assert_eq!(get_service_owner(&service).unwrap(), &owner[..]);
    }

    #[test]
    fn service_owner_invalid_argument_on_bad_data() {
        let service = SplinterService {
            service_id: "sc01".to_string(),
            service_type: "scabbard".to_string(),
            allowed_nodes: vec![],
            arguments: vec![],
        };
        assert_eq!(
            get_service_owner(&service).unwrap_err().argument(),
            "service".to_string(),
        );
    }
}
