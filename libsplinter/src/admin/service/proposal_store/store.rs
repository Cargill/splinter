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

use crate::admin::store::CircuitPredicate;

use crate::admin::service::messages::CircuitProposal;

use super::error::ProposalStoreError;
use super::proposal_iter::ProposalIter;

pub trait ProposalStore: Send + Sync + Clone {
    /// Return an iterator over the proposals in this store. Proposal filters may optionally be
    /// provided.
    fn proposals(&self, filters: Vec<CircuitPredicate>)
        -> Result<ProposalIter, ProposalStoreError>;

    fn proposal(&self, circuit_id: &str) -> Result<Option<CircuitProposal>, ProposalStoreError>;
}
