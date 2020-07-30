// Copyright 2018-2020 Cargill Incorporated
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

//! Provides database migrations for the `DieselRegistry`.

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;

use std::error::Error;
use std::fmt;

#[cfg(feature = "postgres")]
pub use postgres::run_migrations as run_postgres_migrations;
#[cfg(feature = "sqlite")]
pub use sqlite::run_migrations as run_sqlite_migrations;

#[derive(Debug)]
pub struct MigrationError {
    pub context: String,
    pub source: Box<dyn Error>,
}

impl Error for MigrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error applying registry migrations: {}", self.context)
    }
}
