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

use crate::service::instance::ServiceInstance;

/// A service that may be orchestratable.
///
/// This service has several stronger requirements, mainly required moving and sharing a service
/// instance among threads.
pub trait OrchestratableService: ServiceInstance {
    fn clone_box(&self) -> Box<dyn OrchestratableService>;

    fn as_service(&self) -> &dyn ServiceInstance;
}

impl Clone for Box<dyn OrchestratableService> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
