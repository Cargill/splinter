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

#[cfg(feature = "service-message-handler-dispatch")]
mod dispatch;
pub mod instance;
#[cfg(feature = "service-lifecycle-executor")]
mod lifecycle_executor;
#[cfg(feature = "service-message-sender-factory-peer")]
mod network_sender_factory;
#[cfg(feature = "service-timer")]
mod timer;

#[cfg(feature = "service-message-handler-dispatch")]
pub use dispatch::MessageHandlerTaskRunner;
#[cfg(feature = "service-message-handler-dispatch")]
pub use dispatch::ServiceDispatcher;
#[cfg(feature = "service-message-handler-dispatch")]
pub use dispatch::ServiceTypeResolver;
#[cfg(feature = "service-message-handler-dispatch")]
pub use dispatch::SingleThreadedMessageHandlerTaskRunner;
#[cfg(feature = "service-message-handler-dispatch")]
pub use dispatch::{MessageHandlerTaskPool, MessageHandlerTaskPoolBuilder};
#[cfg(all(feature = "diesel", feature = "service-lifecycle-store"))]
pub use lifecycle_executor::DieselLifecycleStore;
#[cfg(all(feature = "service-lifecycle-store", feature = "postgres"))]
pub use lifecycle_executor::PostgresLifecycleStoreFactory;
#[cfg(all(feature = "service-lifecycle-store", feature = "sqlite"))]
pub use lifecycle_executor::SqliteLifecycleStoreFactory;
#[cfg(feature = "service-lifecycle-executor")]
pub use lifecycle_executor::{
    ExecutorAlarm, LifecycleCommand, LifecycleCommandGenerator, LifecycleExecutor,
    LifecycleService, LifecycleServiceBuilder, LifecycleStatus, LifecycleStore,
    LifecycleStoreError, LifecycleStoreFactory,
};
#[cfg(feature = "service-message-sender-factory-peer")]
pub use network_sender_factory::NetworkMessageSenderFactory;
#[cfg(feature = "service-timer")]
pub use timer::{Timer, TimerAlarm};
