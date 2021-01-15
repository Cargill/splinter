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

//! Defines methods and utilities to interact with user tables in the database.

embed_migrations!("./src/biome/migrations/diesel/postgres/migrations");

use diesel::pg::PgConnection;

use crate::database::error::ConnectionError;

/// Run database migrations to create tables defined in the user module
///
/// # Arguments
///
/// * `conn` - Connection to database
///
pub fn run_migrations(conn: &PgConnection) -> Result<(), ConnectionError> {
    embedded_migrations::run(conn).map_err(|err| ConnectionError {
        context: "Failed to embed migrations".to_string(),
        source: Box::new(err),
    })?;

    info!("Successfully applied biome credentials migrations");

    Ok(())
}
