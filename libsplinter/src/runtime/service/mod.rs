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

pub mod instance;
#[cfg(feature = "service-lifecycle-executor")]
mod lifecycle_executor;

#[cfg(all(feature = "diesel", feature = "service-lifecycle-store"))]
pub use lifecycle_executor::DieselLifecycleStore;
#[cfg(all(feature = "service-lifecycle-store", feature = "postgres"))]
pub use lifecycle_executor::PostgresLifecycleStoreFactory;
#[cfg(all(feature = "service-lifecycle-store", feature = "sqlite"))]
pub use lifecycle_executor::SqliteLifecycleStoreFactory;
#[cfg(feature = "service-lifecycle-executor")]
pub use lifecycle_executor::{
    LifecycleCommand, LifecycleCommandGenerator, LifecycleService, LifecycleServiceBuilder,
    LifecycleStatus, LifecycleStore, LifecycleStoreError, LifecycleStoreFactory,
};
