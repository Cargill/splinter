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

//! Database representations used to implement a diesel backend for the `AdminServiceEventStore`.

use std::convert::TryFrom;

use crate::admin::service::event::store::diesel::schema::{
    admin_event_circuit_proposal, admin_event_proposed_circuit, admin_event_proposed_node,
    admin_event_proposed_node_endpoint, admin_event_proposed_service,
    admin_event_proposed_service_argument, admin_event_vote_record, admin_service_event,
};
use crate::admin::service::messages::{
    AdminServiceEvent, AuthorizationType, CircuitProposal, CreateCircuit, DurabilityType,
    PersistenceType, ProposalType, RouteType, Vote, VoteRecord,
};
use crate::error::InvalidStateError;

/// Database model representation of an `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_service_event"]
#[primary_key(id)]
pub struct AdminServiceEventModel {
    pub id: i64,
    pub event_type: String,
    pub data: Option<Vec<u8>>,
}

#[derive(AsChangeset, Insertable, PartialEq, Debug)]
#[table_name = "admin_service_event"]
pub struct NewAdminServiceEventModel<'a> {
    pub event_type: &'a str,
    pub data: Option<&'a [u8]>,
}

/// Database model representation of a `CircuitProposal` from an `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
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

impl From<(i64, &CircuitProposal)> for AdminEventCircuitProposalModel {
    fn from((event_id, proposal): (i64, &CircuitProposal)) -> Self {
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
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
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
        }
    }
}

/// Database model representation of a `VoteRecord` from an `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_event_vote_record"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, voter_node_id)]
pub struct AdminEventVoteRecordModel {
    pub event_id: i64,
    pub public_key: Vec<u8>,
    pub vote: String,
    pub voter_node_id: String,
}

impl TryFrom<&AdminEventVoteRecordModel> for VoteRecord {
    type Error = InvalidStateError;
    fn try_from(vote: &AdminEventVoteRecordModel) -> Result<Self, Self::Error> {
        Ok(VoteRecord {
            public_key: vote.public_key.to_vec(),
            vote: Vote::try_from(vote.vote.clone())?,
            voter_node_id: vote.voter_node_id.to_string(),
        })
    }
}

/// Database model representation of a `AdminEventProposedNode` from an `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_event_proposed_node"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, node_id)]
pub struct AdminEventProposedNodeModel {
    pub event_id: i64,
    pub node_id: String,
}

/// Database model representation of the endpoint values associated with a `ProposedNode` from an
/// `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_event_proposed_node_endpoint"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, node_id, endpoint)]
pub struct AdminEventProposedNodeEndpointModel {
    pub event_id: i64,
    pub node_id: String,
    pub endpoint: String,
}

/// Database model representation of a `ProposedService` from an `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_event_proposed_service"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, service_id)]
pub struct AdminEventProposedServiceModel {
    pub event_id: i64,
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
}

/// Database model representation of the arguments associated with a `ProposedService` from an
/// `AdminServiceEvent`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "admin_event_proposed_service_argument"]
#[belongs_to(AdminServiceEventModel, foreign_key = "event_id")]
#[primary_key(event_id, service_id, key)]
pub struct AdminEventProposedServiceArgumentModel {
    pub event_id: i64,
    pub service_id: String,
    pub key: String,
    pub value: String,
}

// All enums associated with the above structs have TryFrom and From implemented in order to
// translate the enums to a `Text` representation to be stored in the database.

impl TryFrom<String> for Vote {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Accept" => Ok(Vote::Accept),
            "Reject" => Ok(Vote::Reject),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to Vote".into(),
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

impl TryFrom<String> for ProposalType {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Create" => Ok(ProposalType::Create),
            "UpdateRoster" => Ok(ProposalType::UpdateRoster),
            "AddNode" => Ok(ProposalType::AddNode),
            "RemoveNode" => Ok(ProposalType::RemoveNode),
            "Destroy" => Ok(ProposalType::Destroy),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to ProposalType".into(),
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
            ProposalType::Destroy => String::from("Destroy"),
        }
    }
}

impl TryFrom<String> for AuthorizationType {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Trust" => Ok(AuthorizationType::Trust),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to AuthorizationType".into(),
            )),
        }
    }
}

impl From<&AuthorizationType> for String {
    fn from(variant: &AuthorizationType) -> Self {
        match variant {
            AuthorizationType::Trust => String::from("Trust"),
        }
    }
}

impl TryFrom<String> for PersistenceType {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Any" => Ok(PersistenceType::Any),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to PersistenceType".into(),
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

impl TryFrom<String> for DurabilityType {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "NoDurability" => Ok(DurabilityType::NoDurability),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to DurabilityType".into(),
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

impl TryFrom<String> for RouteType {
    type Error = InvalidStateError;
    fn try_from(variant: String) -> Result<Self, Self::Error> {
        match variant.as_ref() {
            "Any" => Ok(RouteType::Any),
            _ => Err(InvalidStateError::with_message(
                "Unable to convert string to RouteType".into(),
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

impl<'a> From<&'a AdminServiceEvent> for NewAdminServiceEventModel<'a> {
    fn from(event: &'a AdminServiceEvent) -> Self {
        match event {
            AdminServiceEvent::ProposalSubmitted(_) => NewAdminServiceEventModel {
                event_type: "ProposalSubmitted",
                data: None,
            },
            AdminServiceEvent::ProposalVote((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalVote",
                data: Some(data),
            },
            AdminServiceEvent::ProposalAccepted((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalAccepted",
                data: Some(data),
            },
            AdminServiceEvent::ProposalRejected((_, data)) => NewAdminServiceEventModel {
                event_type: "ProposalRejected",
                data: Some(data),
            },
            AdminServiceEvent::CircuitReady(_) => NewAdminServiceEventModel {
                event_type: "CircuitReady",
                data: None,
            },
        }
    }
}
