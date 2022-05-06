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

use crate::admin::store::AdminServiceStore;

use super::admin_service_proposals::AdminServiceProposals;
use super::factory::ProposalStoreFactory;
use super::store::ProposalStore;

#[derive(Clone)]
pub struct AdminServiceProposalsFactory {
    admin_store: Box<dyn AdminServiceStore>,
}

impl ProposalStoreFactory for AdminServiceProposalsFactory {
    fn new_proposal_store<'a>(&'a self) -> Box<dyn ProposalStore + 'a> {
        Box::new(AdminServiceProposals::new(self.admin_store.clone()))
    }
}

impl AdminServiceProposalsFactory {
    pub fn new(admin_store: Box<dyn AdminServiceStore>) -> Self {
        Self { admin_store }
    }
}
