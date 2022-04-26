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
use crate::admin::store::CircuitProposal as StoreProposal;

/// An iterator over CircuitProposals, with a well-known count of values.
pub struct ProposalIter {
    inner: Box<dyn ExactSizeIterator<Item = StoreProposal>>,
}

impl ProposalIter {
    pub fn new(iter: Box<dyn ExactSizeIterator<Item = StoreProposal>>) -> Self {
        Self { inner: iter }
    }

    pub fn total(&self) -> usize {
        self.inner.len()
    }
}

impl Iterator for ProposalIter {
    type Item = CircuitProposal;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(CircuitProposal::from)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len(), Some(self.inner.len()))
    }
}
