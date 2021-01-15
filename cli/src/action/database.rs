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

use clap::ArgMatches;

use super::Action;
use crate::error::CliError;
use diesel::{connection::Connection as _, pg::PgConnection};
#[cfg(feature = "database-migrate-biome")]
use splinter::biome::migrations::run_postgres_migrations;

pub struct MigrateAction;

impl Action for MigrateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = if let Some(args) = arg_matches {
            args.value_of("connect")
                .unwrap_or("postgres://admin:admin@localhost:5432/splinterd")
        } else {
            "postgres://admin:admin@localhost:5432/splinterd"
        };

        let connection = PgConnection::establish(url).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to establish database connection to '{}': {}",
                url, err
            ))
        })?;

        #[cfg(feature = "database-migrate-biome")]
        run_postgres_migrations(&connection).map_err(|err| {
            CliError::ActionError(format!("Unable to run Biome migrations: {}", err))
        })?;

        Ok(())
    }
}
