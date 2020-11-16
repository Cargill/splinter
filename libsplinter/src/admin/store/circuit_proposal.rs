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

//! Structs for building circuit proposals

use crate::admin::messages::is_valid_circuit_id;
use crate::error::InvalidStateError;
use crate::protos::admin;

use super::ProposedCircuit;

/// Native representation of a circuit proposal
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircuitProposal {
    proposal_type: ProposalType,
    circuit_id: String,
    circuit_hash: String,
    circuit: ProposedCircuit,
    votes: Vec<VoteRecord>,
    requester: Vec<u8>,
    requester_node_id: String,
}

impl CircuitProposal {
    /// Returns the proposal type of the proposal
    pub fn proposal_type(&self) -> &ProposalType {
        &self.proposal_type
    }

    /// Returns the circuit ID of the circuit in the proposal
    pub fn circuit_id(&self) -> &str {
        &self.circuit_id
    }

    /// Returns the hash of the circuit in the proposal
    pub fn circuit_hash(&self) -> &str {
        &self.circuit_hash
    }

    /// Returns the circuit in the proposal
    pub fn circuit(&self) -> &ProposedCircuit {
        &self.circuit
    }

    /// Returns the list of vote records in the proposal
    pub fn votes(&self) -> &[VoteRecord] {
        &self.votes
    }

    /// Returns the public key that requested the proposal
    pub fn requester(&self) -> &[u8] {
        &self.requester
    }

    /// Returns the node id the requester belongs to
    pub fn requester_node_id(&self) -> &str {
        &self.requester_node_id
    }

    pub fn builder(&self) -> CircuitProposalBuilder {
        CircuitProposalBuilder::new()
            .with_proposal_type(self.proposal_type())
            .with_circuit_id(self.circuit_id())
            .with_circuit_hash(self.circuit_hash())
            .with_circuit(self.circuit())
            .with_votes(self.votes())
            .with_requester(self.requester())
            .with_requester_node_id(self.requester_node_id())
    }

    pub fn from_proto(mut proto: admin::CircuitProposal) -> Result<Self, InvalidStateError> {
        let proposal_type = match proto.get_proposal_type() {
            admin::CircuitProposal_ProposalType::CREATE => ProposalType::Create,
            admin::CircuitProposal_ProposalType::UPDATE_ROSTER => ProposalType::UpdateRoster,
            admin::CircuitProposal_ProposalType::ADD_NODE => ProposalType::AddNode,
            admin::CircuitProposal_ProposalType::REMOVE_NODE => ProposalType::RemoveNode,
            admin::CircuitProposal_ProposalType::DESTROY => ProposalType::Destroy,
            admin::CircuitProposal_ProposalType::UNSET_PROPOSAL_TYPE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `proposal type`".to_string(),
                ));
            }
        };

        let votes = proto
            .take_votes()
            .into_iter()
            .map(VoteRecord::from_proto)
            .collect::<Result<Vec<VoteRecord>, InvalidStateError>>()?;

        Ok(Self {
            proposal_type,
            circuit_id: proto.take_circuit_id(),
            circuit_hash: proto.take_circuit_hash(),
            circuit: ProposedCircuit::from_proto(proto.take_circuit_proposal())?,
            votes,
            requester: proto.take_requester(),
            requester_node_id: proto.take_requester_node_id(),
        })
    }

    pub fn into_proto(self) -> admin::CircuitProposal {
        let proposal_type = match self.proposal_type {
            ProposalType::Create => admin::CircuitProposal_ProposalType::CREATE,
            ProposalType::UpdateRoster => admin::CircuitProposal_ProposalType::UPDATE_ROSTER,
            ProposalType::AddNode => admin::CircuitProposal_ProposalType::ADD_NODE,
            ProposalType::RemoveNode => admin::CircuitProposal_ProposalType::REMOVE_NODE,
            ProposalType::Destroy => admin::CircuitProposal_ProposalType::DESTROY,
        };

        let votes = self
            .votes
            .into_iter()
            .map(|vote| vote.into_proto())
            .collect::<Vec<admin::CircuitProposal_VoteRecord>>();

        let circuit = self.circuit.into_proto();

        let mut proposal = admin::CircuitProposal::new();
        proposal.set_proposal_type(proposal_type);
        proposal.set_circuit_id(self.circuit_id.to_string());
        proposal.set_circuit_hash(self.circuit_hash.to_string());
        proposal.set_circuit_proposal(circuit);
        proposal.set_votes(protobuf::RepeatedField::from_vec(votes));
        proposal.set_requester(self.requester.to_vec());
        proposal.set_requester_node_id(self.requester_node_id);

        proposal
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
    pub fn build(self) -> Result<CircuitProposal, InvalidStateError> {
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

        let proposal_type = self.proposal_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `proposal_type`".to_string(),
            )
        })?;

        let circuit_hash = self.circuit_hash.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `circuit_hash`".to_string(),
            )
        })?;

        let circuit = self.circuit.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `circuit`".to_string())
        })?;

        let mut votes = self.votes.unwrap_or_default();

        votes.sort_by_key(|vote| vote.voter_node_id().to_string());

        let requester = self.requester.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `requester`".to_string(),
            )
        })?;

        let requester_node_id = self.requester_node_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `requester_node_id`".to_string(),
            )
        })?;

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

// Native representation of a vote record for a proposal
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteRecord {
    public_key: Vec<u8>,
    vote: Vote,
    voter_node_id: String,
}

impl VoteRecord {
    /// Returns the public key that submitted the vote
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Returns the vote value of the record
    pub fn vote(&self) -> &Vote {
        &self.vote
    }

    /// Returns the node id the vote record is for
    pub fn voter_node_id(&self) -> &str {
        &self.voter_node_id
    }

    fn from_proto(mut proto: admin::CircuitProposal_VoteRecord) -> Result<Self, InvalidStateError> {
        let vote = match proto.get_vote() {
            admin::CircuitProposalVote_Vote::ACCEPT => Vote::Accept,
            admin::CircuitProposalVote_Vote::REJECT => Vote::Reject,
            admin::CircuitProposalVote_Vote::UNSET_VOTE => {
                return Err(InvalidStateError::with_message(
                    "unable to build, missing field: `vote".to_string(),
                ));
            }
        };

        Ok(Self {
            public_key: proto.take_public_key(),
            vote,
            voter_node_id: proto.take_voter_node_id(),
        })
    }

    fn into_proto(self) -> admin::CircuitProposal_VoteRecord {
        let vote = match self.vote {
            Vote::Accept => admin::CircuitProposalVote_Vote::ACCEPT,
            Vote::Reject => admin::CircuitProposalVote_Vote::REJECT,
        };

        let mut vote_record = admin::CircuitProposal_VoteRecord::new();
        vote_record.set_vote(vote);
        vote_record.set_public_key(self.public_key);
        vote_record.set_voter_node_id(self.voter_node_id);

        vote_record
    }
}

#[derive(Default)]
pub struct VoteRecordBuilder {
    public_key: Option<Vec<u8>>,
    vote: Option<Vote>,
    voter_node_id: Option<String>,
}

impl VoteRecordBuilder {
    pub fn new() -> Self {
        VoteRecordBuilder::default()
    }

    /// Returns the public key that submitted the vote
    pub fn public_key(&self) -> Option<Vec<u8>> {
        self.public_key.clone()
    }

    /// Returns the vote value of the record
    pub fn vote(&self) -> Option<Vote> {
        self.vote.clone()
    }

    /// Returns the node id the vote record is for
    pub fn voter_node_id(&self) -> Option<String> {
        self.voter_node_id.clone()
    }

    pub fn with_public_key(mut self, public_key: &[u8]) -> VoteRecordBuilder {
        self.public_key = Some(public_key.to_vec());
        self
    }

    pub fn with_vote(mut self, vote: &Vote) -> VoteRecordBuilder {
        self.vote = Some(vote.clone());
        self
    }

    pub fn with_voter_node_id(mut self, node_id: &str) -> VoteRecordBuilder {
        self.voter_node_id = Some(node_id.to_string());
        self
    }

    pub fn build(self) -> Result<VoteRecord, InvalidStateError> {
        let public_key = self.public_key.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `public_key`".to_string(),
            )
        })?;

        let vote = self.vote.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `vote`".to_string())
        })?;

        let voter_node_id = self.voter_node_id.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `voter_`".to_string())
        })?;

        Ok(VoteRecord {
            public_key,
            vote,
            voter_node_id,
        })
    }
}

/// Represents a vote, either accept or reject, for a circuit proposal
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vote {
    Accept,
    Reject,
}

/// Represents the of  type change the circuit proposal is for
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalType {
    Create,
    UpdateRoster,
    AddNode,
    RemoveNode,
    Destroy,
}
