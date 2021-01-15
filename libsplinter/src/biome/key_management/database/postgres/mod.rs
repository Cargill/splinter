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

//! Defines methods and utilities to interact with key management tables in the database.

pub(in crate::biome) mod helpers;
pub(in crate::biome) mod models;
pub(super) mod schema;

embed_migrations!("./src/biome/key_management/database/postgres/migrations");

use diesel::pg::PgConnection;

use crate::database::error::DatabaseError;

/// Run database migrations to create tables defined in the key management module
///
/// # Arguments
///
/// * `conn` - Connection to database
///
pub fn run_migrations(conn: &PgConnection) -> Result<(), DatabaseError> {
    embedded_migrations::run(conn).map_err(|err| DatabaseError::ConnectionError(Box::new(err)))?;

    info!("Successfully applied Biome key management migrations");

    Ok(())
}
