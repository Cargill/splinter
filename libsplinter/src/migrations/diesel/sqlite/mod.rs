// Copyright 2018-2021 Cargill Incorporated
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

//! Tools to apply database migrations for SQLite.

embed_migrations!("./src/migrations/diesel/sqlite/migrations");

use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use diesel_migrations::MigrationConnection;

use crate::error::InternalError;

/// Run all pending database migrations.
///
/// # Arguments
///
/// * `conn` - Connection to SQLite database
///
pub fn run_migrations(conn: &SqliteConnection) -> Result<(), InternalError> {
    embedded_migrations::run(conn).map_err(|err| InternalError::from_source(Box::new(err)))?;

    debug!("Successfully applied Splinter SQLite migrations");

    Ok(())
}

/// Get whether there are any pending migrations
///
/// # Arguments
///
/// * `conn` - Connection to SQLite database
///
pub fn any_pending_migrations(conn: &SqliteConnection) -> Result<bool, InternalError> {
    let current_version = conn.latest_run_migration_version().unwrap_or(None);

    // Diesel 1.4 only allows access to the list of migrations via attempting
    // to run the migrations, so we'll do that in a test transaction.
    let latest_version =
        conn.test_transaction::<Result<Option<String>, InternalError>, (), _>(|| {
            Ok(match embedded_migrations::run(conn) {
                Ok(_) => conn
                    .latest_run_migration_version()
                    .map_err(|err| InternalError::from_source(Box::new(err))),
                Err(err) => Err(InternalError::from_source(Box::new(err))),
            })
        })?;

    Ok(current_version == latest_version)
}
