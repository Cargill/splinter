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

//! This module contains an Executor for running lifecycles

#[cfg(feature = "service-lifecycle-store")]
mod store;

#[cfg(all(feature = "service-lifecycle-store", feature = "postgres"))]
pub use store::diesel::factory::PostgresLifecycleStoreFactory;
#[cfg(all(feature = "service-lifecycle-store", feature = "sqlite"))]
pub use store::diesel::factory::SqliteLifecycleStoreFactory;
#[cfg(all(feature = "service-lifecycle-store", feature = "diesel"))]
pub use store::diesel::DieselLifecycleStore;
#[cfg(feature = "service-lifecycle-store")]
pub use store::{
    error::LifecycleStoreError,
    service::{LifecycleCommand, LifecycleService, LifecycleServiceBuilder, LifecycleStatus},
    LifecycleStore, LifecycleStoreFactory,
};
