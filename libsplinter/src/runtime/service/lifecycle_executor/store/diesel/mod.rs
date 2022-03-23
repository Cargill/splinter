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

pub mod factory;
mod models;
mod operations;
mod schema;

use std::sync::{Arc, RwLock};

use diesel::{
    connection::AnsiTransactionManager,
    r2d2::{ConnectionManager, Pool},
};

use crate::runtime::service::{
    LifecycleService, LifecycleStatus, LifecycleStore, LifecycleStoreError, LifecycleStoreFactory,
};
use crate::service::FullyQualifiedServiceId;
use crate::store::pool::ConnectionPool;

use operations::add_service::LifecycleStoreAddServiceOperation as _;
use operations::get_service::LifecycleStoreGetServiceOperation as _;
use operations::list_service::LifecycleStoreListServiceOperation as _;
use operations::remove_service::LifecycleStoreRemoveServiceOperation as _;
use operations::update_service::LifecycleStoreUpdateServiceOperation as _;
use operations::LifecycleStoreOperations;

/// A database-backed LifecycleServiceStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselLifecycleStore<C: diesel::Connection + 'static> {
    connection_pool: ConnectionPool<C>,
}

impl<C: diesel::Connection> DieselLifecycleStore<C> {
    /// Creates a new ` DieselLifecycleStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselLifecycleStore {
            connection_pool: connection_pool.into(),
        }
    }

    /// Create a new ` DieselLifecycleStore` with write exclusivity enabled.
    ///
    /// Write exclusivity is enforced by providing a connection pool that is wrapped in a
    /// [`RwLock`]. This ensures that there may be only one writer, but many readers.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: read-write lock-guarded connection pool for the database
    pub fn new_with_write_exclusivity(
        connection_pool: Arc<RwLock<Pool<ConnectionManager<C>>>>,
    ) -> Self {
        Self {
            connection_pool: connection_pool.into(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselLifecycleStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselLifecycleStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl LifecycleStore for DieselLifecycleStore<diesel::pg::PgConnection> {
    fn add_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).add_service(service))
    }

    fn update_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).update_service(service))
    }

    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).remove_service(service_id))
    }

    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError> {
        self.connection_pool
            .execute_read(|conn| LifecycleStoreOperations::new(conn).get_service(service_id))
    }

    // list services that have the provided LifecycleStatus
    fn list_services(
        &self,
        status: &LifecycleStatus,
    ) -> Result<Vec<LifecycleService>, LifecycleStoreError> {
        self.connection_pool
            .execute_read(|conn| LifecycleStoreOperations::new(conn).list_service(status))
    }
}

#[cfg(feature = "postgres")]
impl DieselLifecycleStore<diesel::pg::PgConnection> {
    pub fn clone_box(&self) -> Box<dyn LifecycleStore + Send> {
        Box::new(self.clone())
    }
}

#[cfg(feature = "sqlite")]
impl LifecycleStore for DieselLifecycleStore<diesel::sqlite::SqliteConnection> {
    fn add_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).add_service(service))
    }

    fn update_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).update_service(service))
    }

    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        self.connection_pool
            .execute_write(|conn| LifecycleStoreOperations::new(conn).remove_service(service_id))
    }

    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError> {
        self.connection_pool
            .execute_read(|conn| LifecycleStoreOperations::new(conn).get_service(service_id))
    }

    // list services that have the provided LifecycleStatus
    fn list_services(
        &self,
        status: &LifecycleStatus,
    ) -> Result<Vec<LifecycleService>, LifecycleStoreError> {
        self.connection_pool
            .execute_read(|conn| LifecycleStoreOperations::new(conn).list_service(status))
    }
}

#[cfg(feature = "sqlite")]
impl DieselLifecycleStore<diesel::sqlite::SqliteConnection> {
    pub fn clone_box(&self) -> Box<dyn LifecycleStore + Send> {
        Box::new(self.clone())
    }
}

pub struct DieselConnectionLifecycleStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    connection: &'a C,
}

impl<'a, C> DieselConnectionLifecycleStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    pub fn new(connection: &'a C) -> Self {
        DieselConnectionLifecycleStore { connection }
    }
}

#[cfg(feature = "postgres")]
impl<'a> LifecycleStore for DieselConnectionLifecycleStore<'a, diesel::pg::PgConnection> {
    fn add_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).add_service(service)
    }

    fn update_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).update_service(service)
    }

    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).remove_service(service_id)
    }

    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).get_service(service_id)
    }

    // list services that have the provided LifecycleStatus
    fn list_services(
        &self,
        status: &LifecycleStatus,
    ) -> Result<Vec<LifecycleService>, LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).list_service(status)
    }
}

#[cfg(feature = "sqlite")]
impl<'a> LifecycleStore for DieselConnectionLifecycleStore<'a, diesel::sqlite::SqliteConnection> {
    fn add_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).add_service(service)
    }

    fn update_service(&self, service: LifecycleService) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).update_service(service)
    }

    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).remove_service(service_id)
    }

    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).get_service(service_id)
    }

    // list services that have the provided LifecycleStatus
    fn list_services(
        &self,
        status: &LifecycleStatus,
    ) -> Result<Vec<LifecycleService>, LifecycleStoreError> {
        LifecycleStoreOperations::new(self.connection).list_service(status)
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
    use crate::runtime::service::{LifecycleCommand, LifecycleServiceBuilder, LifecycleStatus};
    use crate::service::ServiceType;

    // Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
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

    /// Verify that a service can be added to the store correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselLifecycleStore
    /// 3. Create a service
    /// 4. Add service to store
    /// 5. Fetch service from store
    /// 6. Validate fetched service is the same as the service added
    #[test]
    fn test_add_get_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselLifecycleStore::new(pool);

        let service = create_service();

        store
            .add_service(service.clone())
            .expect("Unable to add service");

        let fetched_service = store
            .get_service(service.service_id())
            .expect("Unable to get service")
            .expect("Got None when expecting service");

        assert_eq!(service, fetched_service);
    }

    /// Verify that a service can be updated correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselLifecycleStore
    /// 3. Create a service
    /// 4. Add service to store
    /// 5. Fetch service from store
    /// 6. Validate fetched service is the same as the service added
    /// 7. Update service to have status Initialized
    /// 8. Validate new fetched service is the same as the updated service
    #[test]
    fn test_update_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselLifecycleStore::new(pool);

        let service = create_service();

        store
            .add_service(service.clone())
            .expect("Unable to add service");

        let fetched_service = store
            .get_service(service.service_id())
            .expect("Unable to get service")
            .expect("Got None when expecting service");

        assert_eq!(service, fetched_service);

        let updated_service = service
            .into_builder()
            .with_status(&LifecycleStatus::Complete)
            .build()
            .unwrap();

        store
            .update_service(updated_service.clone())
            .expect("Unable to add service");

        let fetched_service = store
            .get_service(updated_service.service_id())
            .expect("Unable to get service")
            .expect("Got None when expecting service");

        assert_eq!(updated_service, fetched_service);
    }

    /// Verify that a service can be updated correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselLifecycleStore
    /// 3. Create a service
    /// 4. Add service to store
    /// 5. Fetch service from store
    /// 6. Validate fetched service is the same as the service added
    /// 7. Remove the service
    /// 8. Validate None is returned from the store when the service is attempted to be fetched
    #[test]
    fn test_remove_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselLifecycleStore::new(pool);

        let service = create_service();

        store
            .add_service(service.clone())
            .expect("Unable to add service");

        let fetched_service = store
            .get_service(service.service_id())
            .expect("Unable to get service")
            .expect("Got None when expecting service");

        assert_eq!(service, fetched_service);

        store
            .remove_service(service.service_id())
            .expect("Unable to add service");

        let fetched_service = store
            .get_service(service.service_id())
            .expect("Unable to get service");

        assert_eq!(None, fetched_service);
    }

    /// Verify that a service can be updated correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselLifecycleStore
    /// 3. Create a service
    /// 4. Add service to store
    /// 5. Fetch service from store
    /// 6. Validate fetched service is the same as the service added
    /// 7. Update service to have status Initialized
    /// 8. Validate new fetched service is the same as the updated service
    #[test]
    fn test_list_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselLifecycleStore::new(pool);

        let service_1 = create_service();

        store
            .add_service(service_1.clone())
            .expect("Unable to add service");

        let service_2 = create_service();

        store
            .add_service(service_2.clone())
            .expect("Unable to add service");

        let service_3 = create_service();

        // update to initialized state
        let service_3 = service_3
            .into_builder()
            .with_status(&LifecycleStatus::Complete)
            .build()
            .unwrap();

        store
            .add_service(service_3.clone())
            .expect("Unable to add service");

        let services = store
            .list_services(&LifecycleStatus::New)
            .expect("Unable to list services");

        assert!(services.len() == 2);
        assert!(services.contains(&service_1));
        assert!(services.contains(&service_2));
        assert!(!services.contains(&service_3));
    }

    fn create_service() -> LifecycleService {
        let service_id = FullyQualifiedServiceId::new_random();
        LifecycleServiceBuilder::new()
            .with_service_id(&service_id)
            .with_service_type(&ServiceType::new("test").unwrap())
            .with_arguments(&[("arg1".into(), "1".into()), ("arg2".into(), "2".into())])
            .with_command(&LifecycleCommand::Prepare)
            .with_status(&LifecycleStatus::New)
            .build()
            .unwrap()
    }
}
