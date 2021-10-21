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

use std::fmt::Display;
use std::str::FromStr;

#[cfg(feature = "database-postgres")]
use splinter::store::postgres;
#[cfg(feature = "database-sqlite")]
use splinter::store::sqlite;
use splinter::{
    error::{InternalError, InvalidArgumentError},
    store::{memory, StoreFactory},
};

/// Creates a `StoreFactory` backed by the given connection
///
/// # Arguments
///
/// * `connection_uri` - The identifier of the storage connection that will be used by all stores
///   created by the resulting factory
pub fn create_store_factory(
    connection_uri: ConnectionUri,
) -> Result<Box<dyn StoreFactory>, InternalError> {
    match connection_uri {
        ConnectionUri::Memory => Ok(Box::new(memory::MemoryStoreFactory::new()?)),
        #[cfg(feature = "database-postgres")]
        ConnectionUri::Postgres(url) => {
            let pool = postgres::create_postgres_connection_pool(&url)?;
            Ok(Box::new(postgres::PgStoreFactory::new(pool)))
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionUri::Sqlite(conn_str) => {
            let pool = sqlite::create_sqlite_connection_pool(&conn_str)?;
            Ok(Box::new(sqlite::SqliteStoreFactory::new(pool)))
        }
    }
}

/// The possible connection types and identifiers for a `StoreFactory`
pub enum ConnectionUri {
    Memory,
    #[cfg(feature = "database-postgres")]
    Postgres(String),
    #[cfg(feature = "database-sqlite")]
    Sqlite(String),
}

impl Display for ConnectionUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            ConnectionUri::Memory => "memory",
            #[cfg(feature = "database-sqlite")]
            ConnectionUri::Sqlite(sqlite) => sqlite,
            #[cfg(feature = "database-postgres")]
            ConnectionUri::Postgres(pg) => pg,
        };
        write!(f, "{}", string)
    }
}

impl FromStr for ConnectionUri {
    type Err = InvalidArgumentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "memory" => Ok(ConnectionUri::Memory),
            #[cfg(feature = "database-postgres")]
            _ if s.starts_with("postgres://") => Ok(ConnectionUri::Postgres(s.into())),
            #[cfg(feature = "database-sqlite")]
            _ => Ok(ConnectionUri::Sqlite(s.into())),
            #[cfg(not(feature = "database-sqlite"))]
            _ => Err(InvalidArgumentError::new(
                "s".to_string(),
                format!("No compatible connection type: {}", s),
            )),
        }
    }
}
