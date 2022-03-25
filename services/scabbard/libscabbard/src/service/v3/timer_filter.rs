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

use log::info;
use splinter::error::InternalError;
use splinter::service::{FullyQualifiedServiceId, Routable, ServiceType, TimerFilter};

const STATIC_TYPES: &[ServiceType] = &[ServiceType::new_static("scabbard:v3")];

#[derive(Default)]
pub struct ScabbardTimerFilter {}

impl ScabbardTimerFilter {
    pub fn new() -> Self {
        Self {}
    }
}

impl TimerFilter for ScabbardTimerFilter {
    fn filter(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        info!("filtering for scabbard timer");
        Ok(vec![])
    }
}

impl Routable for ScabbardTimerFilter {
    fn service_types(&self) -> &[ServiceType] {
        STATIC_TYPES
    }
}
