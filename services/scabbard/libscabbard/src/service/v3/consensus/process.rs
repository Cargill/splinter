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

use augrim::Process;
use splinter::service::ServiceId;

/// A process identifier for Scabbard Processes.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ScabbardProcess(ServiceId);

impl Process for ScabbardProcess {}

impl From<ServiceId> for ScabbardProcess {
    fn from(service_id: ServiceId) -> Self {
        Self(service_id)
    }
}

impl From<ScabbardProcess> for ServiceId {
    fn from(process: ScabbardProcess) -> Self {
        process.0
    }
}
