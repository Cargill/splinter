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

//! Definations of observers for handling notifications from consensus algorithm.
//!
//! The notification will need to be handled by another part of the system, but the
//! `ConsensusActionRunner` makes no assumptions about how the notifications are delivered.

mod command;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::two_phase::action::Notification;

pub use command::CommandNotifyObserver;

/// Handles notifications from the consensus algorithm to be provided to other components
pub trait NotifyObserver<C> {
    /// Notify components about consensus notification
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification that needs to be handled
    /// * `service_id` - The service ID of of the service the notification is for
    /// * `epoch` - The current epoch of the consensus algorithm
    fn notify(
        &self,
        notification: Notification,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError>;
}
