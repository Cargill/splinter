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

use crate::admin::messages::is_valid_circuit_id;

use super::error::BuilderError;
use super::{CircuitProposal, ProposalType, ProposedCircuit, VoteRecord};

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
