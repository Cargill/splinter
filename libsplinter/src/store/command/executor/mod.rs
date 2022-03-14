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

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod diesel;

use crate::error::InternalError;
use crate::store::command::StoreCommand;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub use self::diesel::DieselStoreCommandExecutor;

/// Provides an API for executing `StoreCommand`s
pub trait StoreCommandExecutor {
    type Context;

    /// Execute each [`StoreCommand`] in `store_commands`
    ///
    /// # Arguments
    ///
    /// * `store_commands` - A list of items that implement the [`StoreCommand`]
    ///   trait
    fn execute<C: StoreCommand<Context = Self::Context>>(
        &self,
        store_commands: Vec<C>,
    ) -> Result<(), InternalError>;
}

impl<C> StoreCommand for Box<dyn StoreCommand<Context = C>> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        (&**self).execute(conn)
    }
}
