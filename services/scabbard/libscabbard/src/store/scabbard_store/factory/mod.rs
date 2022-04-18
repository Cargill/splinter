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

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;

use crate::store::scabbard_store::ScabbardStore;

#[cfg(feature = "postgres")]
pub use postgres::{PgScabbardStoreFactory, PooledPgScabbardStoreFactory};
#[cfg(feature = "sqlite")]
pub use sqlite::{PooledSqliteScabbardStoreFactory, SqliteScabbardStoreFactory};

pub trait ScabbardStoreFactory<C>: Sync + Send {
    fn new_store<'a>(&'a self, conn: &'a C) -> Box<dyn ScabbardStore + 'a>;
}

pub trait PooledScabbardStoreFactory: Send {
    fn new_store(&self) -> Box<dyn ScabbardStore + Send>;

    fn clone_box(&self) -> Box<dyn PooledScabbardStoreFactory>;
}

impl Clone for Box<dyn PooledScabbardStoreFactory> {
    fn clone(&self) -> Box<dyn PooledScabbardStoreFactory> {
        self.clone_box()
    }
}
