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

//! Contains the `Supervisor`, the component that drives transaction execution

mod builder;
mod commands;
mod notifier;
mod notify_observer;

use std::sync::mpsc::Sender;
use std::thread;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::threading::lifecycle::ShutdownHandle;

pub use builder::SupervisorBuilder;
pub use notifier::{SupervisorNotifier, SupervisorNotifierFactory};
pub use notify_observer::SupervisorNotifyObserver;

/// The message used to tell the `Supervisor` there has either been a notification or that
/// it should shutdown.
pub enum SupervisorMessage {
    Notification(FullyQualifiedServiceId),
    Shutdown,
}

/// The component that drives transaction execution
pub struct Supervisor {
    sender: Sender<SupervisorMessage>,
    join_handle: thread::JoinHandle<()>,
}

impl Supervisor {
    /// Return a new `SupervisorNotifierFactory`
    pub fn notifier_factory(&self) -> SupervisorNotifierFactory {
        SupervisorNotifierFactory::new(self.sender.clone())
    }
}

impl ShutdownHandle for Supervisor {
    fn signal_shutdown(&mut self) {
        if self.sender.send(SupervisorMessage::Shutdown).is_err() {
            warn!("Supervisor is no longer running");
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        debug!("Shutting down supervisor thread...");
        self.join_handle.join().map_err(|err| {
            InternalError::with_message(format!(
                "Supervisor thread did not shutdown correctly: {:?}",
                err
            ))
        })?;

        debug!("Shutting down supervisor thread (complete)");
        Ok(())
    }
}
