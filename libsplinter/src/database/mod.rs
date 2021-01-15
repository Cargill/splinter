/*
 * Copyright 2018-2021 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */

pub mod error;

embed_migrations!("./src/biome/migrations/diesel/postgres/migrations");

use std::ops::Deref;

use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};

pub use super::database::error::ConnectionError;

enum InnerConnection {
    Pg(PooledConnection<ConnectionManager<PgConnection>>),
}

pub struct Connection {
    inner: InnerConnection,
}

impl Connection {
    fn new_pg(conn: PooledConnection<ConnectionManager<PgConnection>>) -> Self {
        Connection {
            inner: InnerConnection::Pg(conn),
        }
    }
}

impl Deref for Connection {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            InnerConnection::Pg(conn) => &conn,
        }
    }
}

#[derive(Clone)]
enum InnerPool {
    Pg(Pool<ConnectionManager<PgConnection>>),
}

#[derive(Clone)]
pub struct ConnectionPool {
    inner: InnerPool,
}

impl ConnectionPool {
    pub fn new_pg(database_url: &str) -> Result<Self, ConnectionError> {
        let connection_manager = ConnectionManager::<PgConnection>::new(database_url);
        Ok(ConnectionPool {
            inner: InnerPool::Pg(Pool::builder().build(connection_manager).map_err(|err| {
                ConnectionError {
                    context: "Failed to build connection pool".to_string(),
                    source: Box::new(err),
                }
            })?),
        })
    }

    pub fn get(&self) -> Result<Connection, ConnectionError> {
        match &self.inner {
            InnerPool::Pg(pool) => {
                pool.get()
                    .map(Connection::new_pg)
                    .map_err(|err| ConnectionError {
                        context: "Failed to get Connection from connection pool".to_string(),
                        source: Box::new(err),
                    })
            }
        }
    }
}
