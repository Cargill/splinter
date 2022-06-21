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

//! Contains the a factory for creating a `SupervisorNotifier`

use std::sync::mpsc::Sender;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use super::SupervisorMessage;

/// Creates new `SupervisorNotifier` instances
#[derive(Debug, Clone)]
pub struct SupervisorNotifierFactory {
    sender: Sender<SupervisorMessage>,
}

impl SupervisorNotifierFactory {
    /// Create a new `SupervisorNotifierFactory`
    pub fn new(sender: Sender<SupervisorMessage>) -> Self {
        SupervisorNotifierFactory { sender }
    }

    /// Returns a new `SupervisorNotifier`
    pub fn new_notifier(&self) -> SupervisorNotifier {
        SupervisorNotifier {
            sender: self.sender.clone(),
        }
    }
}

/// Notifies the `Supervisor` when a new notification has been added to state
///
/// When dropped, this struct will send a message to the `Supervisor` to wake it up.
pub struct SupervisorNotifier {
    sender: Sender<SupervisorMessage>,
}

impl SupervisorNotifier {
    pub fn notify(&self, service_id: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        self.sender
            .send(SupervisorMessage::Notification(service_id.clone()))
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
