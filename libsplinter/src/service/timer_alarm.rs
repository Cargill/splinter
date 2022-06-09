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

//! An alarm can be used to prematurely wake all or specific message handlers

use crate::error::InternalError;
use crate::service::{FullyQualifiedServiceId, ServiceType};

pub trait TimerAlarm {
    /// Notify the `Timer` to check all `TimerFilters` for pending work
    fn wake_up_all(&self) -> Result<(), InternalError>;

    /// Notify the `Timer` to check a specific `TimerFilter` for pending work
    ///
    /// # Arguments
    ///
    /// * `service_type` - The service type of the the filter that will be checked
    /// * `service_id` - An optional service ID
    ///
    /// If a service ID is provided, only the `TimerHandler` for that ID will be run. The serivce
    /// ID must be returned from the `TimerFilter` to show there is pending work. If ther service
    /// ID is not returned, no handlers will be run.
    ///
    /// If the service ID is not provided, the handlers for all service IDs returned from the
    /// `TimerFilter` will be run.
    fn wake_up(
        &self,
        service_type: ServiceType<'static>,
        service_id: Option<FullyQualifiedServiceId>,
    ) -> Result<(), InternalError>;
}
