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

use crate::admin::service::messages::CircuitProposal;
use crate::admin::store::AdminServiceStore;
use crate::admin::store::CircuitPredicate;

use super::error::ProposalStoreError;
use super::proposal_iter::ProposalIter;
use super::ProposalStore;

#[derive(Clone)]
pub struct AdminServiceProposals {
    admin_store: Box<dyn AdminServiceStore>,
}

impl AdminServiceProposals {
    pub fn new(admin_store: Box<dyn AdminServiceStore>) -> Self {
        Self { admin_store }
    }
}

impl ProposalStore for AdminServiceProposals {
    fn proposals(
        &self,
        filters: Vec<CircuitPredicate>,
    ) -> Result<ProposalIter, ProposalStoreError> {
        let proposals = self.admin_store.list_proposals(&filters).map_err(|err| {
            ProposalStoreError::from_source("Unable to get proposals", Box::new(err))
        })?;

        Ok(ProposalIter::new(proposals))
    }

    fn proposal(&self, circuit_id: &str) -> Result<Option<CircuitProposal>, ProposalStoreError> {
        self.admin_store
            .get_proposal(circuit_id)
            .map_err(|err| {
                ProposalStoreError::from_source("Unable to get proposal", Box::new(err))
            })?
            .map(|proposal| {
                CircuitProposal::from_proto(proposal.into_proto()).map_err(|err| {
                    ProposalStoreError::from_source(
                        "Unable to convert proposal protobuf to native",
                        Box::new(err),
                    )
                })
            })
            .transpose()
    }
}
