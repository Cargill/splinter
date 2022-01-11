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

use splinter::error::InternalError;
use splinter::registry::RemoteYamlShutdownHandle;
use splinter::threading::lifecycle::ShutdownHandle;

#[derive(Default)]
pub struct RegistryShutdownHandle {
    remote_yaml_shutdown_handles: Vec<RemoteYamlShutdownHandle>,
}

impl RegistryShutdownHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_remote_yaml_shutdown_handle(&mut self, handle: RemoteYamlShutdownHandle) {
        self.remote_yaml_shutdown_handles.push(handle);
    }
}

impl ShutdownHandle for RegistryShutdownHandle {
    fn signal_shutdown(&mut self) {
        self.remote_yaml_shutdown_handles
            .iter_mut()
            .for_each(|handle| handle.signal_shutdown());
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        let mut errors = vec![];
        for handle in self.remote_yaml_shutdown_handles {
            if let Err(err) = handle.wait_for_shutdown() {
                errors.push(err);
            }
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.remove(0)),
            _ => Err(InternalError::with_message(format!(
                "Multiple errors occurred during shutdown: {}",
                errors
                    .into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}
