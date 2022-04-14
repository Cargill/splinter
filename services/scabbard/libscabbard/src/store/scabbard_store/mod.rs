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

pub mod commit;
pub mod context;
mod error;
pub mod service;
pub mod state;

pub(crate) use error::ScabbardStoreError;

use commit::CommitEntry;
use context::ScabbardContext;
use service::ScabbardService;
use splinter::service::FullyQualifiedServiceId;

pub trait ScabbardStore {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError>;
    /// Add a new scabbard service
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError>;
    /// Add a new commit entry
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
    /// Get the commit entry for the specified service_id and epoch
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError>;
    /// Update an existing commit entry
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
}
