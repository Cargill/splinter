// Copyright 2022 Cargill Incorporated
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

//! StoreCommand trait
mod executor;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub use executor::DieselStoreCommandExecutor;
pub use executor::StoreCommandExecutor;

use crate::error::InternalError;

/// Trait for defining a command
///
/// A command will contain information that is to be applied to a database
pub trait StoreCommand {
    type Context;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError>;
}
