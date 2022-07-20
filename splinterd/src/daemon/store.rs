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

#[cfg(any(feature = "database-sqlite", feature = "scabbardv3"))]
use std::sync::Arc;
#[cfg(feature = "database-sqlite")]
use std::sync::RwLock;

#[cfg(feature = "diesel")]
use diesel::r2d2::{ConnectionManager, Pool};
#[cfg(feature = "database-postgres")]
use splinter::store::postgres;
#[cfg(feature = "database-sqlite")]
use splinter::store::sqlite;
use splinter::{
    error::{InternalError, InvalidArgumentError},
    store::StoreFactory,
};
use std::fmt::Display;
use std::str::FromStr;

#[cfg(all(feature = "scabbardv3", feature = "database-postgres"))]
use scabbard::store::PooledPgScabbardStoreFactory;
#[cfg(feature = "scabbardv3")]
use scabbard::store::PooledScabbardStoreFactory;
#[cfg(all(feature = "scabbardv3", feature = "database-sqlite"))]
use scabbard::store::PooledSqliteScabbardStoreFactory;

#[cfg(feature = "service-echo")]
use splinter_echo::store::PooledEchoStoreFactory;
#[cfg(all(feature = "service-echo", feature = "database-postgres"))]
use splinter_echo::store::PooledPgEchoStoreFactory;
#[cfg(all(feature = "service-echo", feature = "database-sqlite"))]
use splinter_echo::store::PooledSqliteEchoStoreFactory;

pub enum ConnectionPool {
    #[cfg(feature = "database-postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    },
    #[cfg(feature = "database-sqlite")]
    Sqlite {
        pool: Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>,
    },
    // This variant is only enabled to such that the compiler does not complain.  It is never
    // constructed.
    #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
    #[allow(dead_code)]
    Unsupported,
}

pub fn create_connection_pool(
    connection_uri: &ConnectionUri,
) -> Result<ConnectionPool, InternalError> {
    match connection_uri {
        #[cfg(feature = "database-postgres")]
        ConnectionUri::Postgres(url) => {
            let pool = postgres::create_postgres_connection_pool(url)?;
            Ok(ConnectionPool::Postgres { pool })
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionUri::Sqlite(conn_str) => {
            let pool = sqlite::create_sqlite_connection_pool_with_write_exclusivity(conn_str)?;
            Ok(ConnectionPool::Sqlite { pool })
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionUri::Memory => {
            let pool = sqlite::create_sqlite_connection_pool_with_write_exclusivity(":memory:")?;
            Ok(ConnectionPool::Sqlite { pool })
        }
        #[cfg(not(feature = "database-sqlite"))]
        ConnectionUri::Memory => Err(InternalError::with_message(
            "Unsupported connection pool type: memory".into(),
        )),
    }
}

/// Creates a `StoreFactory` backed by the given connection
///
/// # Arguments
///
/// * `connection_uri` - The identifier of the storage connection that will be used by all stores
///   created by the resulting factory
pub fn create_store_factory(
    connection_pool: &ConnectionPool,
) -> Result<Box<dyn StoreFactory>, InternalError> {
    match connection_pool {
        #[cfg(feature = "database-postgres")]
        ConnectionPool::Postgres { pool } => {
            Ok(Box::new(postgres::PgStoreFactory::new(pool.clone())))
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionPool::Sqlite { pool } => Ok(Box::new(
            sqlite::SqliteStoreFactory::new_with_write_exclusivity(pool.clone()),
        )),
        #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
        ConnectionPool::Unsupported => Err(InternalError::with_message(
            "Connection pools are unavailable in this configuration".into(),
        )),
    }
}

/// Creates a `ScabbardStoreFactory` backed by the given connection pool
///
/// # Arguments
///
/// * `connection_pool` - the connection pool to use to create the store factory
#[cfg(feature = "scabbardv3")]
pub fn create_scabbard_store_factory(
    connection_pool: &ConnectionPool,
) -> Result<Arc<dyn PooledScabbardStoreFactory>, InternalError> {
    match connection_pool {
        #[cfg(feature = "database-postgres")]
        ConnectionPool::Postgres { pool } => {
            Ok(Arc::new(PooledPgScabbardStoreFactory::new(pool.clone())))
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionPool::Sqlite { pool } => Ok(Arc::new(
            PooledSqliteScabbardStoreFactory::new_with_write_exclusivity(pool.clone()),
        )),
        #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
        ConnectionPool::Unsupported => Err(InternalError::with_message(
            "Connection pools are unavailable in this configuration".into(),
        )),
    }
}

/// Creates a `EchoStoreFactory` backed by the given connection pool
///
/// # Arguments
///
/// * `connection_pool` - the connection pool to use to create the store factory
#[cfg(feature = "service-echo")]
pub fn create_echo_store_factory(
    connection_pool: &ConnectionPool,
) -> Result<Box<dyn PooledEchoStoreFactory>, InternalError> {
    match connection_pool {
        #[cfg(feature = "database-postgres")]
        ConnectionPool::Postgres { pool } => {
            Ok(Box::new(PooledPgEchoStoreFactory::new(pool.clone())))
        }
        #[cfg(feature = "database-sqlite")]
        ConnectionPool::Sqlite { pool } => Ok(Box::new(
            PooledSqliteEchoStoreFactory::new_with_write_exclusivity(pool.clone()),
        )),
        #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
        ConnectionPool::Unsupported => Err(InternalError::with_message(
            "Connection pools are unavailable in this configuration".into(),
        )),
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
