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

mod service_dispatcher;
mod task;
mod task_job_executor;
mod task_single_threaded;
mod type_resolver;

pub use service_dispatcher::ServiceDispatcher;
pub use task::MessageHandlerTaskRunner;
pub use task_job_executor::{MessageHandlerTaskPool, MessageHandlerTaskPoolBuilder};
pub use task_single_threaded::SingleThreadedMessageHandlerTaskRunner;
pub use type_resolver::ServiceTypeResolver;
