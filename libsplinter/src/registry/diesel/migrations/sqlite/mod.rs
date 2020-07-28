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

//! Defines methods and utilities to interact with registry tables in a SQLite database.

embed_migrations!("./src/registry/diesel/migrations/sqlite/migrations");

use diesel::sqlite::SqliteConnection;

use super::MigrationError;

/// Run database migrations to create tables defined by the registry
///
/// # Arguments
///
/// * `conn` - Connection to SQLite database
///
pub fn run_migrations(conn: &SqliteConnection) -> Result<(), MigrationError> {
    embedded_migrations::run(conn).map_err(|err| MigrationError {
        context: "Failed to embed migrations".to_string(),
        source: Box::new(err),
    })?;

    info!("Successfully applied SQLite registry migrations");

    Ok(())
}
