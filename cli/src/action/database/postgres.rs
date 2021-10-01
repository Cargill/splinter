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

use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};
#[cfg(feature = "scabbard-receipt-store")]
use sawtooth::migrations::run_postgres_migrations as run_receipt_store_postgres_migrations;
use splinter::migrations::run_postgres_migrations;
#[cfg(feature = "scabbard-migrations")]
use transact::state::merkle::sql::{backend::PostgresBackend, migration::MigrationManager};

use crate::error::CliError;

macro_rules! conn {
    ($pool:ident) => {
        &*$pool.get().map_err(|_| {
            CliError::ActionError("Failed to get connection for migrations".to_string())
        })?
    };
}

pub fn postgres_migrations(url: &str) -> Result<(), CliError> {
    let connection_manager = ConnectionManager::<PgConnection>::new(url);
    let pool = Pool::builder()
        .max_size(1)
        .build(connection_manager)
        .map_err(|_| CliError::ActionError("Failed to build connection pool".to_string()))?;

    info!("Running migrations against PostgreSQL database: {}", url);
    run_postgres_migrations(conn!(pool)).map_err(|err| {
        CliError::ActionError(format!("Unable to run Postgres migrations: {}", err))
    })?;
    #[cfg(feature = "scabbard-receipt-store")]
    {
        info!(
            "Running migrations against PostgreSQL database for receipt store: {}",
            url
        );
        run_receipt_store_postgres_migrations(conn!(pool)).map_err(|err| {
            CliError::ActionError(format!(
                "Unable to run Postgres migrations for receipt store: {}",
                err
            ))
        })?;
    }

    #[cfg(feature = "scabbard-migrations")]
    {
        scabbard::migrations::run_postgres_migrations(conn!(pool)).map_err(|err| {
            CliError::ActionError(format!(
                "Unable to run Postgres migrations for scabbard: {}",
                err
            ))
        })?;

        let postgres_backend: PostgresBackend = pool.into();
        postgres_backend.run_migrations().map_err(|err| {
            CliError::ActionError(format!(
                "Unable to run Postgres migrations for scabbard merkle state: {}",
                err
            ))
        })?;
    }

    Ok(())
}

#[cfg(not(feature = "sqlite"))]
pub fn get_default_database() -> Result<String, CliError> {
    Ok("postgres://admin:admin@localhost:5432/splinterd".to_string())
}
