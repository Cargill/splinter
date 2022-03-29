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

pub mod orchestrator;
#[cfg(feature = "service-lifecycle-executor")]
pub mod sync;

use std::collections::HashMap;

use crate::error::InternalError;

pub trait LifecycleDispatch: Send {
    // prepare and finalize a service
    fn add_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
        args: Vec<(String, String)>,
    ) -> Result<(), InternalError>;

    fn retire_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError>;

    fn purge_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError>;

    fn shutdown_all_services(&self) -> Result<(), InternalError>;

    fn add_stopped_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
        args: HashMap<String, String>,
    ) -> Result<(), InternalError>;
}
