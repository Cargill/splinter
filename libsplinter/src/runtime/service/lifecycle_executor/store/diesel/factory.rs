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

use super::{DieselConnectionLifecycleStore, LifecycleStore, LifecycleStoreFactory};

#[cfg(feature = "sqlite")]
pub struct SqliteLifecycleStoreFactory;

#[cfg(feature = "sqlite")]
impl LifecycleStoreFactory<diesel::sqlite::SqliteConnection> for SqliteLifecycleStoreFactory {
    fn new_store<'a>(
        &'a self,
        conn: &'a diesel::sqlite::SqliteConnection,
    ) -> Box<dyn LifecycleStore + 'a> {
        Box::new(DieselConnectionLifecycleStore::new(conn))
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresLifecycleStoreFactory;

#[cfg(feature = "postgres")]
impl LifecycleStoreFactory<diesel::pg::PgConnection> for PostgresLifecycleStoreFactory {
    fn new_store<'a>(&'a self, conn: &'a diesel::pg::PgConnection) -> Box<dyn LifecycleStore + 'a> {
        Box::new(DieselConnectionLifecycleStore::new(conn))
    }
}
