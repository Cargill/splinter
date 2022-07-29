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

use splinter::admin::messages::{
    CircuitProposal, CreateCircuit, ProposalType, SplinterNode, SplinterService, Vote, VoteRecord,
};
use splinter_rest_api_common::paging::v1::Paging;

use crate::hex::as_hex;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct ListProposalsResponse<'a> {
    pub data: Vec<ProposalResponse<'a>>,
    pub paging: Paging,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct ProposalResponse<'a> {
    pub proposal_type: &'a str,
    pub circuit_id: &'a str,
    pub circuit_hash: &'a str,
    pub circuit: CircuitResponse<'a>,
    pub votes: Vec<VoteResponse<'a>>,
    #[serde(serialize_with = "as_hex")]
    pub requester: &'a [u8],
    pub requester_node_id: &'a str,
}

impl<'a> From<&'a CircuitProposal> for ProposalResponse<'a> {
    fn from(proposal: &'a CircuitProposal) -> Self {
        let proposal_type = match proposal.proposal_type {
            ProposalType::Create => "Create",
            ProposalType::UpdateRoster => "UpdateRoster",
            ProposalType::AddNode => "AddNode",
            ProposalType::RemoveNode => "RemoveNode",
            ProposalType::Disband => "Disband",
        };

        Self {
            proposal_type,
            circuit_id: &proposal.circuit_id,
            circuit_hash: &proposal.circuit_hash,
            circuit: (&proposal.circuit).into(),
            votes: proposal.votes.iter().map(VoteResponse::from).collect(),
            requester: &proposal.requester,
            requester_node_id: &proposal.requester_node_id,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct VoteResponse<'a> {
    #[serde(serialize_with = "as_hex")]
    pub public_key: &'a [u8],
    pub vote: &'a str,
    pub voter_node_id: &'a str,
}

impl<'a> From<&'a VoteRecord> for VoteResponse<'a> {
    fn from(record: &'a VoteRecord) -> Self {
        let vote = match record.vote {
            Vote::Accept => "Accept",
            Vote::Reject => "Reject",
        };

        Self {
            public_key: &record.public_key,
            vote,
            voter_node_id: &record.voter_node_id,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct CircuitResponse<'a> {
    pub circuit_id: &'a str,
    pub members: Vec<NodeResponse<'a>>,
    pub roster: Vec<ServiceResponse<'a>>,
    pub management_type: &'a str,
    #[serde(serialize_with = "as_hex")]
    pub application_metadata: &'a [u8],
    pub comments: String,
}

impl<'a> From<&'a CreateCircuit> for CircuitResponse<'a> {
    fn from(circuit: &'a CreateCircuit) -> Self {
        Self {
            circuit_id: &circuit.circuit_id,
            members: circuit.members.iter().map(NodeResponse::from).collect(),
            roster: circuit.roster.iter().map(ServiceResponse::from).collect(),
            management_type: &circuit.circuit_management_type,
            application_metadata: &circuit.application_metadata,
            comments: circuit.comments.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct NodeResponse<'a> {
    pub node_id: &'a str,
    pub endpoints: &'a [String],
}

impl<'a> From<&'a SplinterNode> for NodeResponse<'a> {
    fn from(node: &'a SplinterNode) -> Self {
        Self {
            node_id: &node.node_id,
            endpoints: &node.endpoints,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct ServiceResponse<'a> {
    pub service_id: &'a str,
    pub service_type: &'a str,
    pub allowed_nodes: &'a [String],
    pub arguments: &'a [(String, String)],
}

impl<'a> From<&'a SplinterService> for ServiceResponse<'a> {
    fn from(service: &'a SplinterService) -> Self {
        Self {
            service_id: &service.service_id,
            service_type: &service.service_type,
            allowed_nodes: &service.allowed_nodes,
            arguments: &service.arguments,
        }
    }
}
