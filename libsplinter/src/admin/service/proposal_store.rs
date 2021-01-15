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

use std::sync::{Arc, Mutex};

use super::messages::CircuitProposal;
use super::shared::AdminServiceShared;

/// A filter that matches on aspects of a proposal.
///
/// Each variant applies to a different field on the proposal or its circuit defition.
#[derive(Debug)]
pub enum ProposalFilter {
    /// Matches any proposals whose circuits have the given management type.
    WithManagementType(String),
    /// Matches any proposals whose circuits have the given node as a member.
    WithMember(String),
}

impl ProposalFilter {
    /// Returns true if the given proposal matches the filter criteria, false otherwise.
    pub fn matches(&self, proposal: &CircuitProposal) -> bool {
        match self {
            ProposalFilter::WithManagementType(ref management_type) => {
                &proposal.circuit.circuit_management_type == management_type
            }
            ProposalFilter::WithMember(ref member_id) => proposal
                .circuit
                .members
                .iter()
                .any(|member| &member.node_id == member_id),
        }
    }
}

pub trait ProposalStore: Send + Sync + Clone {
    /// Return an iterator over the proposals in this store. Proposal filters may optionally be
    /// provided.
    fn proposals(&self, filters: Vec<ProposalFilter>) -> Result<ProposalIter, ProposalStoreError>;

    fn proposal(&self, circuit_id: &str) -> Result<Option<CircuitProposal>, ProposalStoreError>;
}

#[derive(Debug)]
pub struct ProposalStoreError {
    context: String,
    source: Option<Box<dyn std::error::Error + Send + 'static>>,
}

impl std::error::Error for ProposalStoreError {}

impl ProposalStoreError {
    pub fn new(context: &str) -> Self {
        Self {
            context: context.into(),
            source: None,
        }
    }

    pub fn from_source<T: std::error::Error + Send + 'static>(context: &str, source: T) -> Self {
        Self {
            context: context.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn context(&self) -> &str {
        &self.context
    }
}

impl std::fmt::Display for ProposalStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(ref source) = self.source {
            write!(
                f,
                "ProposalStoreError: Source: {} Context: {}",
                source, self.context
            )
        } else {
            write!(f, "ProposalStoreError: Context {}", self.context)
        }
    }
}

#[derive(Clone)]
pub(super) struct AdminServiceProposals {
    shared: Arc<Mutex<AdminServiceShared>>,
}

impl AdminServiceProposals {
    pub fn new(shared: &Arc<Mutex<AdminServiceShared>>) -> Self {
        Self {
            shared: Arc::clone(shared),
        }
    }
}

impl ProposalStore for AdminServiceProposals {
    fn proposals(&self, filters: Vec<ProposalFilter>) -> Result<ProposalIter, ProposalStoreError> {
        let proposals = self
            .shared
            .lock()
            .map_err(|_| ProposalStoreError::new("Admin shared lock was lock poisoned"))?
            .get_proposals();

        let total = proposals
            .iter()
            .filter(|(_, proposal)| filters.iter().all(|filter| filter.matches(proposal)))
            .count();

        let iter = Box::new(proposals.into_iter().filter_map(move |(_, proposal)| {
            if filters.iter().all(|filter| filter.matches(&proposal)) {
                Some(proposal)
            } else {
                None
            }
        }));

        Ok(ProposalIter::new(iter, total))
    }

    fn proposal(&self, circuit_id: &str) -> Result<Option<CircuitProposal>, ProposalStoreError> {
        self.shared
            .lock()
            .map_err(|_| ProposalStoreError::new("Admin shared lock was lock poisoned"))?
            .get_proposal(circuit_id)
            .map_err(|err| {
                ProposalStoreError::from_source("Unable to get proposal", Box::new(err))
            })?
            .map(|proto| {
                CircuitProposal::from_proto(proto).map_err(|err| {
                    ProposalStoreError::from_source(
                        "Unable to convert proposal protobuf to native",
                        Box::new(err),
                    )
                })
            })
            .transpose()
    }
}

/// An iterator over CircuitProposals, with a well-known count of values.
pub struct ProposalIter {
    inner: Box<dyn Iterator<Item = CircuitProposal>>,
    size: usize,
}

impl ProposalIter {
    pub fn new(iter: Box<dyn Iterator<Item = CircuitProposal>>, count: usize) -> Self {
        Self {
            inner: iter,
            size: count,
        }
    }

    pub fn total(&self) -> usize {
        self.size
    }
}

impl Iterator for ProposalIter {
    type Item = CircuitProposal;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.size, Some(self.size))
    }
}
