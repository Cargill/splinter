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

//! Database representations used to implement a diesel backend for the `AdminServiceStore`.
//! These structs differ slightly from their associated native representation to conform to
//! the requirements for storing data with a diesel backend.

use std::convert::TryFrom;
use std::io::Write;

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    expression::{helper_types::AsExprOf, AsExpression},
    serialize::{self, Output, ToSql},
    sql_types::SmallInt,
};

use crate::admin::service::messages::{self, CreateCircuit};
use crate::admin::store::diesel::schema::{
    admin_event_circuit_proposal, admin_event_proposed_circuit, admin_event_proposed_node,
    admin_event_proposed_node_endpoint, admin_event_proposed_service,
    admin_event_proposed_service_argument, admin_event_vote_record, admin_service_event,
};
use crate::admin::store::diesel::schema::{
    circuit, circuit_member, circuit_proposal, node_endpoint, proposed_circuit, proposed_node,
    proposed_node_endpoint, proposed_service, proposed_service_argument, service, service_argument,
    vote_record,
};
use crate::admin::store::error::AdminServiceStoreError;
use crate::admin::store::{AdminServiceEvent, AdminServiceEventBuilder, EventType};
use crate::admin::store::{
    AuthorizationType, CircuitStatus, DurabilityType, PersistenceType, ProposalType, RouteType,
    Vote, VoteRecord, VoteRecordBuilder,
};
use crate::admin::store::{Circuit, CircuitProposal, ProposedCircuit};
use crate::error::{InternalError, InvalidStateError};
use crate::public_key::PublicKey;

/// Database model representation of a `CircuitProposal`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "circuit_proposal"]
#[primary_key(circuit_id)]
pub struct CircuitProposalModel {
    pub proposal_type: String,
    pub circuit_id: String,
    pub circuit_hash: String,
    pub requester: Vec<u8>,
    pub requester_node_id: String,
}

impl From<&CircuitProposal> for CircuitProposalModel {
    fn from(proposal: &CircuitProposal) -> Self {
        CircuitProposalModel {
            proposal_type: String::from(proposal.proposal_type()),
            circuit_id: proposal.circuit_id().into(),
            circuit_hash: proposal.circuit_hash().into(),
            requester: proposal.requester().as_slice().to_vec(),
            requester_node_id: proposal.requester_node_id().into(),
        }
    }
}

/// Database model representation of a `ProposedCircuit`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "proposed_circuit"]
#[belongs_to(CircuitProposalModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id)]
pub struct ProposedCircuitModel {
    pub circuit_id: String,
    pub authorization_type: String,
    pub persistence: String,
    pub durability: String,
    pub routes: String,
    pub circuit_management_type: String,
    pub application_metadata: Option<Vec<u8>>,
    pub comments: Option<String>,
    pub display_name: Option<String>,
    pub circuit_version: i32,
    pub circuit_status: CircuitStatusModel,
}

impl From<&ProposedCircuit> for ProposedCircuitModel {
    fn from(proposed_circuit: &ProposedCircuit) -> Self {
        ProposedCircuitModel {
            circuit_id: proposed_circuit.circuit_id().into(),
            authorization_type: String::from(proposed_circuit.authorization_type()),
            persistence: String::from(proposed_circuit.persistence()),
            durability: String::from(proposed_circuit.durability()),
            routes: String::from(proposed_circuit.routes()),
            circuit_management_type: proposed_circuit.circuit_management_type().into(),
            application_metadata: proposed_circuit.application_metadata().clone(),
            comments: proposed_circuit.comments().clone(),
            display_name: proposed_circuit.display_name().clone(),
            circuit_version: proposed_circuit.circuit_version(),
            circuit_status: CircuitStatusModel::from(proposed_circuit.circuit_status()),
        }
    }
}

/// Database model representation of a `VoteRecord`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "vote_record"]
#[belongs_to(CircuitProposalModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, voter_node_id)]
pub struct VoteRecordModel {
    pub circuit_id: String,
    pub public_key: Vec<u8>,
    pub vote: String,
    pub voter_node_id: String,
    pub position: i32,
}

impl TryFrom<&CircuitProposal> for Vec<VoteRecordModel> {
    type Error = AdminServiceStoreError;

    fn try_from(proposal: &CircuitProposal) -> Result<Self, Self::Error> {
        proposal
            .votes()
            .iter()
            .enumerate()
            .map(|(idx, vote)| {
                Ok(VoteRecordModel {
                    circuit_id: proposal.circuit_id().into(),
                    public_key: vote.public_key().as_slice().to_vec(),
                    vote: String::from(vote.vote()),
                    voter_node_id: vote.voter_node_id().into(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect::<Result<Vec<VoteRecordModel>, AdminServiceStoreError>>()
    }
}

impl TryFrom<&VoteRecordModel> for VoteRecord {
    type Error = AdminServiceStoreError;
    fn try_from(vote: &VoteRecordModel) -> Result<Self, Self::Error> {
        VoteRecordBuilder::new()
            .with_public_key(&PublicKey::from_bytes(vote.public_key.to_vec()))
            .with_vote(&Vote::try_from(vote.vote.clone())?)
            .with_voter_node_id(&vote.voter_node_id)
            .build()
            .map_err(AdminServiceStoreError::InvalidStateError)
    }
}

/// Database model representation of a `ProposedNode`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "proposed_node"]
#[belongs_to(ProposedCircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, node_id)]
pub struct ProposedNodeModel {
    pub circuit_id: String,
    pub node_id: String,
    pub position: i32,
    pub public_key: Option<Vec<u8>>,
}

impl TryFrom<&ProposedCircuit> for Vec<ProposedNodeModel> {
    type Error = AdminServiceStoreError;

    fn try_from(proposed_circuit: &ProposedCircuit) -> Result<Self, Self::Error> {
        proposed_circuit
            .members()
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                Ok(ProposedNodeModel {
                    circuit_id: proposed_circuit.circuit_id().into(),
                    node_id: node.node_id().into(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                    public_key: node
                        .public_key()
                        .clone()
                        .map(|public_key| public_key.into_bytes()),
                })
            })
            .collect::<Result<Vec<ProposedNodeModel>, AdminServiceStoreError>>()
    }
}

/// Database model representation of the endpoint values associated with a `ProposedNode`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "proposed_node_endpoint"]
#[belongs_to(ProposedCircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, node_id, endpoint)]
pub struct ProposedNodeEndpointModel {
    pub node_id: String,
    pub circuit_id: String,
    pub endpoint: String,
    pub position: i32,
}

impl TryFrom<&ProposedCircuit> for Vec<ProposedNodeEndpointModel> {
    type Error = AdminServiceStoreError;

    fn try_from(proposed_circuit: &ProposedCircuit) -> Result<Self, Self::Error> {
        let mut endpoint_models = Vec::new();
        for node in proposed_circuit.members() {
            endpoint_models.extend(
                node.endpoints()
                    .iter()
                    .enumerate()
                    .map(|(idx, endpoint)| {
                        Ok(ProposedNodeEndpointModel {
                            node_id: node.node_id().into(),
                            circuit_id: proposed_circuit.circuit_id().into(),
                            endpoint: endpoint.clone(),
                            position: i32::try_from(idx).map_err(|_| {
                                AdminServiceStoreError::InternalError(InternalError::with_message(
                                    "Unable to convert index into i32".to_string(),
                                ))
                            })?,
                        })
                    })
                    .collect::<Result<Vec<ProposedNodeEndpointModel>, AdminServiceStoreError>>()?,
            );
        }
        Ok(endpoint_models)
    }
}

/// Database model representation of a `ProposedService`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "proposed_service"]
#[belongs_to(ProposedCircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, service_id)]
pub struct ProposedServiceModel {
    pub circuit_id: String,
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub position: i32,
}

impl TryFrom<&ProposedCircuit> for Vec<ProposedServiceModel> {
    type Error = AdminServiceStoreError;

    fn try_from(proposed_circuit: &ProposedCircuit) -> Result<Self, Self::Error> {
        proposed_circuit
            .roster()
            .iter()
            .enumerate()
            .map(|(idx, service)| {
                Ok(ProposedServiceModel {
                    circuit_id: proposed_circuit.circuit_id().into(),
                    service_id: service.service_id().into(),
                    service_type: service.service_type().into(),
                    node_id: service.node_id().into(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect::<Result<Vec<ProposedServiceModel>, AdminServiceStoreError>>()
    }
}

/// Database model representation of the arguments associated with a `ProposedService`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "proposed_service_argument"]
#[belongs_to(ProposedCircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, service_id, key)]
pub struct ProposedServiceArgumentModel {
    pub circuit_id: String,
    pub service_id: String,
    pub key: String,
    pub value: String,
    pub position: i32,
}

impl TryFrom<&ProposedCircuit> for Vec<ProposedServiceArgumentModel> {
    type Error = AdminServiceStoreError;

    fn try_from(proposed_circuit: &ProposedCircuit) -> Result<Self, Self::Error> {
        let mut service_arguments = Vec::new();
        for service in proposed_circuit.roster() {
            service_arguments.extend(
                service
                    .arguments()
                    .iter()
                    .enumerate()
                    .map(|(idx, (key, value))| {
                        Ok(ProposedServiceArgumentModel {
                            circuit_id: proposed_circuit.circuit_id().into(),
                            service_id: service.service_id().into(),
                            key: key.into(),
                            value: value.into(),
                            position: i32::try_from(idx).map_err(|_| {
                                AdminServiceStoreError::InternalError(InternalError::with_message(
                                    "Unable to convert index into i32".to_string(),
                                ))
                            })?,
                        })
                    })
                    .collect::<Result<Vec<ProposedServiceArgumentModel>, AdminServiceStoreError>>(
                    )?,
            );
        }
        Ok(service_arguments)
    }
}

/// Database model representation of `Service`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "service"]
#[belongs_to(CircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, service_id)]
pub struct ServiceModel {
    pub circuit_id: String,
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub position: i32,
}

impl TryFrom<&Circuit> for Vec<ServiceModel> {
    type Error = AdminServiceStoreError;

    fn try_from(circuit: &Circuit) -> Result<Self, Self::Error> {
        circuit
            .roster()
            .iter()
            .enumerate()
            .map(|(idx, service)| {
                Ok(ServiceModel {
                    circuit_id: circuit.circuit_id().into(),
                    service_id: service.service_id().into(),
                    service_type: service.service_type().into(),
                    node_id: service.node_id().into(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect::<Result<Vec<ServiceModel>, AdminServiceStoreError>>()
    }
}

/// Database model representation of the arguments in a `Service`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "service_argument"]
#[belongs_to(CircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, service_id, key)]
pub struct ServiceArgumentModel {
    pub circuit_id: String,
    pub service_id: String,
    pub key: String,
    pub value: String,
    pub position: i32,
}

impl TryFrom<&Circuit> for Vec<ServiceArgumentModel> {
    type Error = AdminServiceStoreError;

    fn try_from(circuit: &Circuit) -> Result<Self, Self::Error> {
        let mut service_arguments = Vec::new();
        for service in circuit.roster() {
            service_arguments.extend(
                service
                    .arguments()
                    .iter()
                    .enumerate()
                    .map(|(idx, (key, value))| {
                        Ok(ServiceArgumentModel {
                            circuit_id: circuit.circuit_id().into(),
                            service_id: service.service_id().into(),
                            key: key.clone(),
                            value: value.clone(),
                            position: i32::try_from(idx).map_err(|_| {
                                AdminServiceStoreError::InternalError(InternalError::with_message(
                                    "Unable to convert index into i32".to_string(),
                                ))
                            })?,
                        })
                    })
                    .collect::<Result<Vec<ServiceArgumentModel>, AdminServiceStoreError>>()?,
            );
        }
        Ok(service_arguments)
    }
}

/// Database model representation of a `Circuit`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "circuit"]
#[primary_key(circuit_id)]
pub struct CircuitModel {
    pub circuit_id: String,
    pub authorization_type: String,
    pub persistence: String,
    pub durability: String,
    pub routes: String,
    pub circuit_management_type: String,
    pub display_name: Option<String>,
    pub circuit_version: i32,
    pub circuit_status: CircuitStatusModel,
}

impl From<&Circuit> for CircuitModel {
    fn from(circuit: &Circuit) -> Self {
        CircuitModel {
            circuit_id: circuit.circuit_id().into(),
            authorization_type: String::from(circuit.authorization_type()),
            persistence: String::from(circuit.persistence()),
            durability: String::from(circuit.durability()),
            routes: String::from(circuit.routes()),
            circuit_management_type: circuit.circuit_management_type().into(),
            display_name: circuit.display_name().clone(),
            circuit_version: circuit.circuit_version(),
            circuit_status: CircuitStatusModel::from(circuit.circuit_status()),
        }
    }
}

/// Database model representation of the `members` of a `Circuit`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "circuit_member"]
#[belongs_to(CircuitModel, foreign_key = "circuit_id")]
#[primary_key(circuit_id, node_id)]
pub struct CircuitMemberModel {
    pub circuit_id: String,
    pub node_id: String,
    pub position: i32,
    pub public_key: Option<Vec<u8>>,
}

impl TryFrom<&Circuit> for Vec<CircuitMemberModel> {
    type Error = AdminServiceStoreError;

    fn try_from(circuit: &Circuit) -> Result<Self, Self::Error> {
        circuit
            .members()
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                Ok(CircuitMemberModel {
                    circuit_id: circuit.circuit_id().into(),
                    node_id: node.node_id().into(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                    public_key: node
                        .public_key()
                        .clone()
                        .map(|public_key| public_key.into_bytes()),
                })
            })
            .collect::<Result<Vec<CircuitMemberModel>, AdminServiceStoreError>>()
    }
}

/// Database model representation of the endpoint values associated with a `Circuit` member `node_id`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "node_endpoint"]
#[primary_key(node_id, endpoint)]
pub struct NodeEndpointModel {
    pub node_id: String,
    pub endpoint: String,
}

/// Database model representation of an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_service_event"]
#[primary_key(id)]
pub struct AdminServiceEventModel {
    pub id: i64,
    pub event_type: String,
    pub data: Option<Vec<u8>>,
}

#[derive(AsChangeset, Insertable, PartialEq, Eq, Debug)]
#[table_name = "admin_service_event"]
pub struct NewAdminServiceEventModel<'a> {
    pub event_type: &'a str,
    pub data: Option<&'a [u8]>,
}

/// Database model representation of a `CircuitProposal` from an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_circuit_proposal"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct AdminEventCircuitProposalModel {
    pub event_id: i64,
    pub proposal_type: String,
    pub circuit_id: String,
    pub circuit_hash: String,
    pub requester: Vec<u8>,
    pub requester_node_id: String,
}

impl From<(i64, &messages::CircuitProposal)> for AdminEventCircuitProposalModel {
    fn from((event_id, proposal): (i64, &messages::CircuitProposal)) -> Self {
        AdminEventCircuitProposalModel {
            event_id,
            proposal_type: String::from(&proposal.proposal_type),
            circuit_id: proposal.circuit_id.to_string(),
            circuit_hash: proposal.circuit_hash.to_string(),
            requester: proposal.requester.to_vec(),
            requester_node_id: proposal.requester_node_id.to_string(),
        }
    }
}

/// Database model representation of a `CreateCircuit` from an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_proposed_circuit"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct AdminEventProposedCircuitModel {
    pub event_id: i64,
    pub circuit_id: String,
    pub authorization_type: String,
    pub persistence: String,
    pub durability: String,
    pub routes: String,
    pub circuit_management_type: String,
    pub application_metadata: Option<Vec<u8>>,
    pub comments: Option<String>,
    pub display_name: Option<String>,
    pub circuit_version: i32,
    pub circuit_status: CircuitStatusModel,
}

impl From<(i64, &CreateCircuit)> for AdminEventProposedCircuitModel {
    fn from((event_id, create_circuit): (i64, &CreateCircuit)) -> Self {
        let application_metadata = if create_circuit.application_metadata.is_empty() {
            None
        } else {
            Some(create_circuit.application_metadata.to_vec())
        };

        AdminEventProposedCircuitModel {
            event_id,
            circuit_id: create_circuit.circuit_id.to_string(),
            authorization_type: String::from(&create_circuit.authorization_type),
            persistence: String::from(&create_circuit.persistence),
            durability: String::from(&create_circuit.durability),
            routes: String::from(&create_circuit.routes),
            circuit_management_type: create_circuit.circuit_management_type.to_string(),
            application_metadata,
            comments: create_circuit.comments.clone(),
            display_name: create_circuit.display_name.clone(),
            circuit_version: create_circuit.circuit_version,
            circuit_status: CircuitStatusModel::from(&create_circuit.circuit_status),
        }
    }
}

/// Database model representation of a `VoteRecord` from an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_vote_record"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, voter_node_id)]
pub struct AdminEventVoteRecordModel {
    pub event_id: i64,
    pub public_key: Vec<u8>,
    pub vote: String,
    pub voter_node_id: String,
    pub position: i32,
}

impl AdminEventVoteRecordModel {
    // Creates a list of `AdminEventVoteRecordModel` from a `CircuitProposal` associated with
    // an `AdminServiceEvent`
    pub(super) fn list_from_proposal_with_id(
        event_id: i64,
        proposal: &messages::CircuitProposal,
    ) -> Result<Vec<AdminEventVoteRecordModel>, AdminServiceStoreError> {
        proposal
            .votes
            .iter()
            .enumerate()
            .map(|(idx, vote)| {
                Ok(AdminEventVoteRecordModel {
                    event_id,
                    public_key: vote.public_key.to_vec(),
                    vote: String::from(&vote.vote),
                    voter_node_id: vote.voter_node_id.to_string(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect()
    }
}

impl TryFrom<&AdminEventVoteRecordModel> for VoteRecord {
    type Error = InvalidStateError;
    fn try_from(
        admin_event_vote_record_model: &AdminEventVoteRecordModel,
    ) -> Result<Self, Self::Error> {
        VoteRecordBuilder::new()
            .with_public_key(&PublicKey::from_bytes(
                admin_event_vote_record_model.public_key.to_vec(),
            ))
            .with_vote(
                &Vote::try_from(admin_event_vote_record_model.vote.clone()).map_err(|_| {
                    InvalidStateError::with_message("Unable to convert string to Vote".into())
                })?,
            )
            .with_voter_node_id(&admin_event_vote_record_model.voter_node_id)
            .build()
    }
}

/// Database model representation of a `AdminEventProposedNode` from an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_proposed_node"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, node_id)]
pub struct AdminEventProposedNodeModel {
    pub event_id: i64,
    pub node_id: String,
    pub position: i32,
}

impl AdminEventProposedNodeModel {
    // Creates a list of `AdminEventProposedNodeModel` from a `CircuitProposal` associated with
    // an `AdminServiceEvent`
    pub(super) fn list_from_proposal_with_id(
        event_id: i64,
        proposal: &messages::CircuitProposal,
    ) -> Result<Vec<AdminEventProposedNodeModel>, AdminServiceStoreError> {
        proposal
            .circuit
            .members
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                Ok(AdminEventProposedNodeModel {
                    event_id,
                    node_id: node.node_id.to_string(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect()
    }
}

/// Database model representation of the endpoint values associated with a `ProposedNode` from an
/// `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_proposed_node_endpoint"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, node_id, endpoint)]
pub struct AdminEventProposedNodeEndpointModel {
    pub event_id: i64,
    pub node_id: String,
    pub endpoint: String,
    pub position: i32,
}

impl AdminEventProposedNodeEndpointModel {
    // Creates a list of `AdminEventProposedNodeEndpointModel` from a `CircuitProposal` associated
    // with an `AdminServiceEvent`
    pub(super) fn list_from_proposal_with_id(
        event_id: i64,
        proposal: &messages::CircuitProposal,
    ) -> Result<Vec<AdminEventProposedNodeEndpointModel>, AdminServiceStoreError> {
        let mut endpoint_models = Vec::new();
        for node in &proposal.circuit.members {
            endpoint_models.extend(
                node.endpoints
                    .iter()
                    .enumerate()
                    .map(|(idx, endpoint)| Ok(AdminEventProposedNodeEndpointModel {
                        event_id,
                        node_id: node.node_id.to_string(),
                        endpoint: endpoint.to_string(),
                        position: i32::try_from(idx).map_err(|_| {
                            AdminServiceStoreError::InternalError(InternalError::with_message(
                                "Unable to convert index into i32".to_string(),
                            ))
                        })?,
                    }))
                    .collect::<Result<
                        Vec<AdminEventProposedNodeEndpointModel>, AdminServiceStoreError>
                    >()?,
            );
        }
        Ok(endpoint_models)
    }
}

/// Database model representation of a `ProposedService` from an `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_proposed_service"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, service_id)]
pub struct AdminEventProposedServiceModel {
    pub event_id: i64,
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub position: i32,
}

impl AdminEventProposedServiceModel {
    // Creates a list of `AdminEventProposedServiceModel` from a `CircuitProposal` associated
    // with an `AdminServiceEvent`
    pub(super) fn list_from_proposal_with_id(
        event_id: i64,
        proposal: &messages::CircuitProposal,
    ) -> Result<Vec<AdminEventProposedServiceModel>, AdminServiceStoreError> {
        proposal
            .circuit
            .roster
            .iter()
            .enumerate()
            .map(|(idx, service)| {
                Ok(AdminEventProposedServiceModel {
                    event_id,
                    service_id: service.service_id.to_string(),
                    service_type: service.service_type.to_string(),
                    node_id: service
                        .allowed_nodes
                        .get(0)
                        .ok_or_else(|| {
                            AdminServiceStoreError::InvalidStateError(
                                InvalidStateError::with_message(
                                    "Must contain 1 node ID".to_string(),
                                ),
                            )
                        })?
                        .to_string(),
                    position: i32::try_from(idx).map_err(|_| {
                        AdminServiceStoreError::InternalError(InternalError::with_message(
                            "Unable to convert index into i32".to_string(),
                        ))
                    })?,
                })
            })
            .collect::<Result<Vec<AdminEventProposedServiceModel>, AdminServiceStoreError>>()
    }
}

/// Database model representation of the arguments associated with a `ProposedService` from an
/// `AdminServiceEvent`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "admin_event_proposed_service_argument"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, service_id, key)]
pub struct AdminEventProposedServiceArgumentModel {
    pub event_id: i64,
    pub service_id: String,
    pub key: String,
    pub value: String,
    pub position: i32,
}

impl AdminEventProposedServiceArgumentModel {
    // Creates a list of `AdminEventProposedServiceArgumentModel` from a `CircuitProposal` associated
    // with an `AdminServiceEvent`
    pub(super) fn list_from_proposal_with_id(
        event_id: i64,
        proposal: &messages::CircuitProposal,
    ) -> Result<Vec<AdminEventProposedServiceArgumentModel>, AdminServiceStoreError> {
        let mut service_arguments = Vec::new();
        for service in &proposal.circuit.roster {
            service_arguments.extend(
                service
                    .arguments
                    .iter()
                    .enumerate()
                    .map(|(idx, (key, value))| {
                        Ok(AdminEventProposedServiceArgumentModel {
                            event_id,
                            service_id: service.service_id.to_string(),
                            key: key.into(),
                            value: value.into(),
                            position: i32::try_from(idx).map_err(|_| {
                                AdminServiceStoreError::InternalError(
                                    InternalError::with_message(
                                        "Unable to convert index into i32".to_string(),
                                    ),
                                )
                            })?,
                        })
                    })
                    .collect::<Result<
                        Vec<AdminEventProposedServiceArgumentModel>,
                        AdminServiceStoreError,
                    >>()?,
            );
        }
        Ok(service_arguments)
    }
}

impl<'a> From<&'a messages::AdminServiceEvent> for NewAdminServiceEventModel<'a> {
    fn from(event: &'a messages::AdminServiceEvent) -> Self {
        match event {
            messages::AdminServiceEvent::ProposalSubmitted(_) => NewAdminServiceEventModel {
                event_type: "ProposalSubmitted",
                data: None,
            },
            messages::AdminServiceEvent::ProposalVote((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalVote",
                data: Some(data),
            },
            messages::AdminServiceEvent::ProposalAccepted((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalAccepted",
                data: Some(data),
            },
            messages::AdminServiceEvent::ProposalRejected((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalRejected",
                data: Some(data),
            },
            messages::AdminServiceEvent::CircuitReady(_) => NewAdminServiceEventModel {
                event_type: "CircuitReady",
                data: None,
            },
            messages::AdminServiceEvent::CircuitDisbanded(_) => NewAdminServiceEventModel {
                event_type: "CircuitDisbanded",
                data: None,
            },
        }
    }
}

impl TryFrom<(AdminServiceEventModel, CircuitProposal)> for AdminServiceEvent {
    type Error = AdminServiceStoreError;

    fn try_from(
        (event_model, proposal): (AdminServiceEventModel, CircuitProposal),
    ) -> Result<Self, Self::Error> {
        match (event_model.event_type.as_ref(), event_model.data) {
            ("ProposalSubmitted", None) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::ProposalSubmitted)
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            ("ProposalVote", Some(requester)) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::ProposalVote { requester })
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            ("ProposalAccepted", Some(requester)) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::ProposalAccepted { requester })
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            ("ProposalRejected", Some(requester)) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::ProposalRejected { requester })
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            ("CircuitReady", None) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::CircuitReady)
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            ("CircuitDisbanded", None) => AdminServiceEventBuilder::new()
                .with_event_id(event_model.id)
                .with_event_type(&EventType::CircuitDisbanded)
                .with_proposal(&proposal)
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message(
                    "Unable to convert AdminServiceEventModel to AdminServiceEvent".into(),
                ),
            )),
        }
    }
}

// All enums associated with the above structs have TryFrom and From implemented in order to
// translate the enums to a `Text` representation to be stored in the database.

impl TryFrom<String> for Vote {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Accept" => Ok(Vote::Accept),
            "Reject" => Ok(Vote::Reject),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message("Unable to convert string to Vote".into()),
            )),
        }
    }
}

impl From<&Vote> for String {
    fn from(variant: &Vote) -> Self {
        match variant {
            Vote::Accept => String::from("Accept"),
            Vote::Reject => String::from("Reject"),
        }
    }
}

impl From<&messages::Vote> for String {
    fn from(variant: &messages::Vote) -> Self {
        match variant {
            messages::Vote::Accept => String::from("Accept"),
            messages::Vote::Reject => String::from("Reject"),
        }
    }
}

impl TryFrom<String> for ProposalType {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Create" => Ok(ProposalType::Create),
            "UpdateRoster" => Ok(ProposalType::UpdateRoster),
            "AddNode" => Ok(ProposalType::AddNode),
            "RemoveNode" => Ok(ProposalType::RemoveNode),
            "Disband" => Ok(ProposalType::Disband),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message("Unable to convert string to ProposalType".into()),
            )),
        }
    }
}

impl From<&ProposalType> for String {
    fn from(variant: &ProposalType) -> Self {
        match variant {
            ProposalType::Create => String::from("Create"),
            ProposalType::UpdateRoster => String::from("UpdateRoster"),
            ProposalType::AddNode => String::from("AddNode"),
            ProposalType::RemoveNode => String::from("RemoveNode"),
            ProposalType::Disband => String::from("Disband"),
        }
    }
}

impl From<&messages::ProposalType> for String {
    fn from(variant: &messages::ProposalType) -> Self {
        match variant {
            messages::ProposalType::Create => String::from("Create"),
            messages::ProposalType::UpdateRoster => String::from("UpdateRoster"),
            messages::ProposalType::AddNode => String::from("AddNode"),
            messages::ProposalType::RemoveNode => String::from("RemoveNode"),
            messages::ProposalType::Disband => String::from("Disband"),
        }
    }
}

impl TryFrom<String> for AuthorizationType {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Trust" => Ok(AuthorizationType::Trust),
            "Challenge" => Ok(AuthorizationType::Challenge),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message(
                    "Unable to convert string to AuthorizationType".into(),
                ),
            )),
        }
    }
}

impl From<&AuthorizationType> for String {
    fn from(variant: &AuthorizationType) -> Self {
        match variant {
            AuthorizationType::Trust => String::from("Trust"),
            AuthorizationType::Challenge => String::from("Challenge"),
        }
    }
}

impl From<&messages::AuthorizationType> for String {
    fn from(variant: &messages::AuthorizationType) -> Self {
        match variant {
            messages::AuthorizationType::Trust => String::from("Trust"),
            messages::AuthorizationType::Challenge => String::from("Challenge"),
        }
    }
}

impl TryFrom<String> for PersistenceType {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Any" => Ok(PersistenceType::Any),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message(
                    "Unable to convert string to PersistenceType".into(),
                ),
            )),
        }
    }
}

impl From<&PersistenceType> for String {
    fn from(variant: &PersistenceType) -> Self {
        match variant {
            PersistenceType::Any => String::from("Any"),
        }
    }
}

impl From<&messages::PersistenceType> for String {
    fn from(variant: &messages::PersistenceType) -> Self {
        match variant {
            messages::PersistenceType::Any => String::from("Any"),
        }
    }
}

impl TryFrom<String> for DurabilityType {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "NoDurability" => Ok(DurabilityType::NoDurability),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message(
                    "Unable to convert string to DurabilityType".into(),
                ),
            )),
        }
    }
}

impl From<&DurabilityType> for String {
    fn from(variant: &DurabilityType) -> Self {
        match variant {
            DurabilityType::NoDurability => String::from("NoDurability"),
        }
    }
}

impl From<&messages::DurabilityType> for String {
    fn from(variant: &messages::DurabilityType) -> Self {
        match variant {
            messages::DurabilityType::NoDurability => String::from("NoDurability"),
        }
    }
}

impl TryFrom<String> for RouteType {
    type Error = AdminServiceStoreError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Any" => Ok(RouteType::Any),
            _ => Err(AdminServiceStoreError::InvalidStateError(
                InvalidStateError::with_message("Unable to convert string to RouteType".into()),
            )),
        }
    }
}

impl From<&RouteType> for String {
    fn from(variant: &RouteType) -> Self {
        match variant {
            RouteType::Any => String::from("Any"),
        }
    }
}

impl From<&messages::RouteType> for String {
    fn from(variant: &messages::RouteType) -> Self {
        match variant {
            messages::RouteType::Any => String::from("Any"),
        }
    }
}

#[repr(i16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, FromSqlRow)]
pub enum CircuitStatusModel {
    Active = 1,
    Disbanded = 2,
    Abandoned = 3,
}

impl From<&CircuitStatus> for CircuitStatusModel {
    fn from(store_status: &CircuitStatus) -> Self {
        match *store_status {
            CircuitStatus::Active => CircuitStatusModel::Active,
            CircuitStatus::Disbanded => CircuitStatusModel::Disbanded,
            CircuitStatus::Abandoned => CircuitStatusModel::Abandoned,
        }
    }
}

impl From<&messages::CircuitStatus> for CircuitStatusModel {
    fn from(messages_status: &messages::CircuitStatus) -> Self {
        match *messages_status {
            messages::CircuitStatus::Active => CircuitStatusModel::Active,
            messages::CircuitStatus::Disbanded => CircuitStatusModel::Disbanded,
            messages::CircuitStatus::Abandoned => CircuitStatusModel::Abandoned,
        }
    }
}

impl From<&CircuitStatusModel> for CircuitStatus {
    fn from(status_model: &CircuitStatusModel) -> Self {
        match *status_model {
            CircuitStatusModel::Active => CircuitStatus::Active,
            CircuitStatusModel::Disbanded => CircuitStatus::Disbanded,
            CircuitStatusModel::Abandoned => CircuitStatus::Abandoned,
        }
    }
}

impl From<&CircuitStatusModel> for messages::CircuitStatus {
    fn from(status_model: &CircuitStatusModel) -> Self {
        match *status_model {
            CircuitStatusModel::Active => messages::CircuitStatus::Active,
            CircuitStatusModel::Disbanded => messages::CircuitStatus::Disbanded,
            CircuitStatusModel::Abandoned => messages::CircuitStatus::Abandoned,
        }
    }
}

impl<DB> ToSql<SmallInt, DB> for CircuitStatusModel
where
    DB: Backend,
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        (*self as i16).to_sql(out)
    }
}

impl AsExpression<SmallInt> for CircuitStatusModel {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression(self as i16)
    }
}

impl<'a> AsExpression<SmallInt> for &'a CircuitStatusModel {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression((*self) as i16)
    }
}

impl<DB> FromSql<SmallInt, DB> for CircuitStatusModel
where
    DB: Backend,
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match i16::from_sql(bytes)? {
            1 => Ok(CircuitStatusModel::Active),
            2 => Ok(CircuitStatusModel::Disbanded),
            3 => Ok(CircuitStatusModel::Abandoned),
            int => Err(format!("Invalid circuit status {}", int).into()),
        }
    }
}
