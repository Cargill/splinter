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

//! Stores required for lifecycle execution

pub mod error;
pub mod service;

use crate::service::FullyQualifiedServiceId;

use self::error::LifecycleStoreError;
use self::service::{LifecycleCommand, LifecycleService, LifecycleServiceBuilder, LifecycleStatus};

pub trait LifecycleStore {
    fn add_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError>;

    fn update_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError>;

    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError>;

    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError>;

    // list services that have the provided LifecycleStatus
    fn list_services(
        &self,
        status: &LifecycleStatus,
    ) -> Result<Vec<LifecycleService>, LifecycleStoreError>;
}
