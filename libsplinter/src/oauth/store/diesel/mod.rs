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

//! Diesel-backed InflightOAuthRequestStore implementation.

mod models;
mod operations;
mod schema;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    result,
};

use crate::error::{ConstraintViolationError, ConstraintViolationType, InternalError};
use crate::oauth::PendingAuthorization;

use super::{InflightOAuthRequestStore, InflightOAuthRequestStoreError};

use operations::insert_request::InflightOAuthRequestStoreInsertRequestOperation as _;
use operations::remove_request::InflightOAuthRequestStoreRemoveRequestOperation as _;
use operations::InflightOAuthRequestOperations;

/// A Diesel-backed InflightOAuthRequestStore
pub struct DieselInflightOAuthRequestStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection + 'static> DieselInflightOAuthRequestStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl InflightOAuthRequestStore
    for DieselInflightOAuthRequestStore<diesel::sqlite::SqliteConnection>
{
    fn insert_request(
        &self,
        request_id: String,
        pending_authorization: PendingAuthorization,
    ) -> Result<(), InflightOAuthRequestStoreError> {
        let connection = self
            .connection_pool
            .get()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        InflightOAuthRequestOperations::new(&*connection).insert_request(
            models::OAuthInflightRequest {
                id: request_id,
                pkce_verifier: pending_authorization.pkce_verifier,
                client_redirect_url: pending_authorization.client_redirect_url,
            },
        )
    }

    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<PendingAuthorization>, InflightOAuthRequestStoreError> {
        let connection = self
            .connection_pool
            .get()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        InflightOAuthRequestOperations::new(&*connection)
            .remove_request(request_id)
            .map(|opt_request| opt_request.map(PendingAuthorization::from))
    }

    fn clone_box(&self) -> Box<dyn InflightOAuthRequestStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(feature = "oauth-inflight-request-store-postgres")]
impl InflightOAuthRequestStore for DieselInflightOAuthRequestStore<diesel::pg::PgConnection> {
    fn insert_request(
        &self,
        request_id: String,
        pending_authorization: PendingAuthorization,
    ) -> Result<(), InflightOAuthRequestStoreError> {
        let connection = self
            .connection_pool
            .get()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        InflightOAuthRequestOperations::new(&*connection).insert_request(
            models::OAuthInflightRequest {
                id: request_id,
                pkce_verifier: pending_authorization.pkce_verifier,
                client_redirect_url: pending_authorization.client_redirect_url,
            },
        )
    }

    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<PendingAuthorization>, InflightOAuthRequestStoreError> {
        let connection = self
            .connection_pool
            .get()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        InflightOAuthRequestOperations::new(&*connection)
            .remove_request(request_id)
            .map(|opt_request| opt_request.map(PendingAuthorization::from))
    }

    fn clone_box(&self) -> Box<dyn InflightOAuthRequestStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

impl From<models::OAuthInflightRequest> for PendingAuthorization {
    fn from(model: models::OAuthInflightRequest) -> Self {
        PendingAuthorization {
            pkce_verifier: model.pkce_verifier,
            client_redirect_url: model.client_redirect_url,
        }
    }
}

impl From<diesel::r2d2::PoolError> for InflightOAuthRequestStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        InflightOAuthRequestStoreError::InternalError(InternalError::from_source(Box::new(err)))
    }
}

impl From<result::Error> for InflightOAuthRequestStoreError {
    fn from(err: result::Error) -> Self {
        match err {
            result::Error::DatabaseError(ref kind, _) => match kind {
                result::DatabaseErrorKind::UniqueViolation => {
                    InflightOAuthRequestStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                _ => InflightOAuthRequestStoreError::InternalError(InternalError::from_source(
                    Box::new(err),
                )),
            },
            _ => InflightOAuthRequestStoreError::InternalError(InternalError::from_source(
                Box::new(err),
            )),
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    use crate::migrations::run_sqlite_migrations;
    use crate::oauth::store::tests::{
        test_duplicate_id_insert, test_request_store_insert_and_remove,
    };

    #[test]
    fn sqlite_insert_request_and_remove() {
        let pool = create_connection_pool_and_migrate();
        let inflight_request_store = DieselInflightOAuthRequestStore::new(pool);
        test_request_store_insert_and_remove(&inflight_request_store);
    }

    #[test]
    fn sqlite_duplicate_id_insert() {
        let pool = create_connection_pool_and_migrate();
        let inflight_request_store = DieselInflightOAuthRequestStore::new(pool);
        test_duplicate_id_insert(&inflight_request_store);
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection insures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
