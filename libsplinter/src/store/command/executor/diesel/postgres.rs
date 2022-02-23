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

use diesel::{pg::PgConnection, Connection};

use crate::error::InternalError;
use crate::store::command::{DieselStoreCommandExecutor, StoreCommand, StoreCommandExecutor};

impl StoreCommandExecutor for DieselStoreCommandExecutor<PgConnection> {
    type Context = PgConnection;

    fn execute<C: StoreCommand<Context = Self::Context>>(
        &self,
        store_commands: Vec<C>,
    ) -> Result<(), InternalError> {
        let conn = &*self
            .conn
            .get()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        conn.transaction::<(), InternalError, _>(|| {
            for cmd in store_commands {
                cmd.execute(conn)?;
            }
            Ok(())
        })
    }
}
