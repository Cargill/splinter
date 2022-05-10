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

//! Defination of an updater for handling context updates and setting alarms consensus.
//!
//! The `ConsensusActionRunner` makes no assumptions about how the context are updated
//! or how alarms are set.

mod store;

use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::context::ConsensusContext;

pub use store::ScabbardStoreContextUpdater;

/// Handles updating consensus context and alarms
pub trait ContextUpdater<C> {
    /// Update context and alarms
    ///
    /// # Arguments
    ///
    /// * `context` - The context to be updated
    /// * `service_id` - The service ID of of the service the notification is for
    /// * `alarm` - The alarm to update
    fn update(
        &self,
        context: ConsensusContext,
        service_id: &FullyQualifiedServiceId,
        alarm: Option<SystemTime>,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError>;
}
