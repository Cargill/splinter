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

use std::collections::HashMap;
use std::{
    thread,
    time::{Duration, Instant},
};

use crate::error::InternalError;
use crate::runtime::service::{
    ExecutorAlarm, LifecycleCommand, LifecycleServiceBuilder, LifecycleStatus, LifecycleStore,
};
use crate::service::{FullyQualifiedServiceId, ServiceType};

use super::LifecycleDispatch;

const TIME_BETWEEN_DATABASE_CHECK: Duration = Duration::from_secs(1);

pub struct SyncLifecycleInterface {
    store: Box<dyn LifecycleStore + Send>,
    alarm: Box<dyn ExecutorAlarm>,
    supported_types: Vec<String>,
    time_to_wait: Duration,
}

impl SyncLifecycleInterface {
    pub fn new(
        store: Box<dyn LifecycleStore + Send>,
        alarm: Box<dyn ExecutorAlarm>,
        supported_types: Vec<String>,
        time_to_wait: Duration,
    ) -> Self {
        SyncLifecycleInterface {
            store,
            alarm,
            supported_types,
            time_to_wait,
        }
    }
}

impl LifecycleDispatch for SyncLifecycleInterface {
    // prepare and finalize a service
    fn add_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
        args: Vec<(String, String)>,
    ) -> Result<(), InternalError> {
        let service_id =
            FullyQualifiedServiceId::new_from_string(format!("{}::{}", circuit_id, service_id))
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let service_type = ServiceType::new(service_type)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        if !self.supported_types.contains(&service_type.to_string()) {
            trace!(
                "Ignoring call to add service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        if self
            .store
            .get_service(&service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .is_some()
        {
            trace!(
                "Skip adding service: {}::{} ({}), already exists",
                circuit_id,
                service_id,
                service_type,
            );
            return Ok(());
        }

        debug!(
            "Adding service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let mut service = LifecycleServiceBuilder::new()
            .with_service_id(&service_id)
            .with_service_type(&service_type)
            .with_arguments(&args)
            .with_status(&LifecycleStatus::New)
            .with_command(&LifecycleCommand::Prepare)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.store
            .add_service(service.clone())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.alarm
            .wake_up(service_type.clone(), Some(service_id.clone()))?;

        // Wait for the service to be prepared
        let instant = Instant::now();
        while service.status() == &LifecycleStatus::New {
            if instant.elapsed() > self.time_to_wait {
                return Err(InternalError::with_message(format!(
                    "Service {} was not prepared in time",
                    service_id
                )));
            }

            thread::sleep(TIME_BETWEEN_DATABASE_CHECK);
            service = self
                .store
                .get_service(&service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .ok_or_else(|| {
                    InternalError::with_message(format!("Unable to get service {}", service_id))
                })?;
        }

        // Now that the service is prepared, finalize
        service = service
            .into_builder()
            .with_status(&LifecycleStatus::New)
            .with_command(&LifecycleCommand::Finalize)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.store
            .update_service(service.clone())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.alarm
            .wake_up(service_type.clone(), Some(service_id.clone()))?;

        // Wait for the service to be prepared
        let instant = Instant::now();
        while service.status() == &LifecycleStatus::New {
            if instant.elapsed() > self.time_to_wait {
                return Err(InternalError::with_message(format!(
                    "Service {} was not finalized in time",
                    service_id
                )));
            }

            thread::sleep(TIME_BETWEEN_DATABASE_CHECK);
            service = self
                .store
                .get_service(&service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .ok_or_else(|| {
                    InternalError::with_message(format!("Unable to get service {}", service_id))
                })?;
        }

        Ok(())
    }

    fn retire_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError> {
        let service_id =
            FullyQualifiedServiceId::new_from_string(format!("{}::{}", circuit_id, service_id))
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let service_type = ServiceType::new(service_type)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        if !self.supported_types.contains(&service_type.to_string()) {
            trace!(
                "Ignoring call to retire service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Retiring service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service = self
            .store
            .get_service(&service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!("Unable to get service {}", service_id))
            })?;

        let mut service = service
            .into_builder()
            .with_status(&LifecycleStatus::New)
            .with_command(&LifecycleCommand::Retire)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.store
            .update_service(service.clone())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.alarm.wake_up(service_type, Some(service_id.clone()))?;

        // Wait for the service to be prepared
        let instant = Instant::now();
        while service.status() == &LifecycleStatus::New {
            if instant.elapsed() > self.time_to_wait {
                return Err(InternalError::with_message(format!(
                    "Service {} was not retired in time",
                    service_id
                )));
            }

            thread::sleep(TIME_BETWEEN_DATABASE_CHECK);
            service = self
                .store
                .get_service(&service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .ok_or_else(|| {
                    InternalError::with_message(format!("Unable to get service {}", service_id))
                })?;
        }

        Ok(())
    }

    fn purge_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError> {
        let service_id =
            FullyQualifiedServiceId::new_from_string(format!("{}::{}", circuit_id, service_id))
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let service_type = ServiceType::new(service_type)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        if !self.supported_types.contains(&service_type.to_string()) {
            trace!(
                "Ignoring call to purge service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Purging service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service = self
            .store
            .get_service(&service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!("Unable to get service {}", service_id))
            })?;

        let service = service
            .into_builder()
            .with_status(&LifecycleStatus::New)
            .with_command(&LifecycleCommand::Purge)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.store
            .update_service(service.clone())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        self.alarm.wake_up(service_type, Some(service_id.clone()))?;

        // Wait for the service to be prepared
        let mut pending_service = Some(service);

        let instant = Instant::now();
        while pending_service.is_some() {
            if instant.elapsed() > self.time_to_wait {
                return Err(InternalError::with_message(format!(
                    "Service {} was not purged in time",
                    service_id
                )));
            }
            thread::sleep(TIME_BETWEEN_DATABASE_CHECK);
            pending_service = self
                .store
                .get_service(&service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
        }

        Ok(())
    }

    fn shutdown_all_services(&self) -> Result<(), InternalError> {
        // not required for Lifecycle implementation
        Ok(())
    }

    fn add_stopped_service(
        &self,
        _circuit_id: &str,
        _service_id: &str,
        _service_type: &str,
        _args: HashMap<String, String>,
    ) -> Result<(), InternalError> {
        // not required for Lifecycle implementation
        Ok(())
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::marker::PhantomData;
    use std::sync::Arc;

    use diesel::r2d2::{ConnectionManager, Pool};

    use crate::migrations::run_sqlite_migrations;
    use crate::runtime::service::{
        DieselLifecycleStore, LifecycleCommand, LifecycleCommandGenerator, LifecycleExecutor,
        LifecycleService, LifecycleServiceBuilder, LifecycleStatus, LifecycleStore,
        LifecycleStoreFactory, SqliteLifecycleStoreFactory,
    };
    use crate::service::Lifecycle;
    use crate::store::command::{DieselStoreCommandExecutor, StoreCommand};
    use crate::threading::lifecycle::ShutdownHandle;

    // Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate(
    ) -> Pool<ConnectionManager<diesel::sqlite::SqliteConnection>> {
        let connection_manager =
            ConnectionManager::<diesel::sqlite::SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
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

    struct TestCommand<C> {
        _context: PhantomData<C>,
    }

    impl<C> TestCommand<C> {
        fn new() -> Self {
            TestCommand {
                _context: PhantomData,
            }
        }
    }

    impl<C> StoreCommand for TestCommand<C> {
        type Context = C;

        fn execute(&self, _conn: &Self::Context) -> Result<(), InternalError> {
            Ok(())
        }
    }

    struct TestLifecycle<K: 'static> {
        _context: PhantomData<K>,
    }

    impl<K: 'static> TestLifecycle<K> {
        fn new() -> Self {
            TestLifecycle {
                _context: PhantomData,
            }
        }
    }

    impl<K> Lifecycle<K> for TestLifecycle<K> {
        type Arguments = Vec<(String, String)>;

        fn command_to_prepare(
            &self,
            _service: FullyQualifiedServiceId,
            _arguments: Self::Arguments,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand::new()))
        }

        fn command_to_finalize(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand::new()))
        }

        fn command_to_retire(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand::new()))
        }

        fn command_to_purge(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand::new()))
        }
    }

    fn create_executor(
        wake_up_interval: Duration,
    ) -> (
        LifecycleExecutor<DieselStoreCommandExecutor<diesel::sqlite::SqliteConnection>>,
        DieselLifecycleStore<diesel::sqlite::SqliteConnection>,
    ) {
        let pool = create_connection_pool_and_migrate();

        let mut lifecycles: HashMap<
            ServiceType<'static>,
            Box<
                dyn Lifecycle<diesel::sqlite::SqliteConnection, Arguments = Vec<(String, String)>>
                    + Send,
            >,
        > = HashMap::new();

        let test_lifecycle = TestLifecycle::new();

        lifecycles.insert(ServiceType::new("test").unwrap(), Box::new(test_lifecycle));

        let store = DieselLifecycleStore::new(pool.clone());
        let store_factory: Arc<(dyn LifecycleStoreFactory<diesel::sqlite::SqliteConnection>)> =
            Arc::new(SqliteLifecycleStoreFactory);

        let command_generator = LifecycleCommandGenerator::new(store_factory);
        let command_executor = DieselStoreCommandExecutor::new(pool.clone());

        let executor = LifecycleExecutor::new(
            wake_up_interval,
            lifecycles,
            store.clone_box(),
            command_generator,
            command_executor,
        )
        .unwrap();

        (executor, store)
    }

    // Verify that the SyncLifecycleInterface will properly wait for a service to be prepared and
    // finalized before returning
    //
    // 1. Setup the LifecycleExecutor
    // 2. Call add_service and verify it returns Ok
    // 3. Check that an associated service is in the LifecycleStore and that the service is
    //    finalized.
    #[test]
    fn test_add_service() {
        let (mut executor, store) = create_executor(Duration::from_secs(10));

        let alarm = executor.alarm();
        let interface = SyncLifecycleInterface::new(
            store.clone_box(),
            alarm,
            vec!["test".to_string()],
            Duration::from_secs(10),
        );

        interface
            .add_service(
                "ABCDE-12345",
                "a000",
                "test",
                vec![("arg1".into(), "1".into()), ("arg2".into(), "2".into())],
            )
            .unwrap();

        let service = store
            .get_service(&FullyQualifiedServiceId::new_from_string("ABCDE-12345::a000").unwrap())
            .expect("unable to get service")
            .expect("service was none");

        assert_eq!(service.command(), &LifecycleCommand::Finalize);
        assert_eq!(service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    // Verify that the SyncLifecycleInterface will properly wait for a service to be retired
    // before returning
    //
    // 1. Setup the LifecycleExecutor
    // 2. Add a finalized service to the LifecycleStore
    // 2. Call retire_service and verify it returns Ok
    // 3. Check that an associated service is in the LifecycleStore and that the service is
    //    retired.
    #[test]
    fn test_retire_service() {
        let (mut executor, store) = create_executor(Duration::from_secs(10));

        let alarm = executor.alarm();
        let interface = SyncLifecycleInterface::new(
            store.clone_box(),
            alarm,
            vec!["test".to_string()],
            Duration::from_secs(10),
        );

        let service = create_service();

        let service = service
            .into_builder()
            .with_command(&LifecycleCommand::Finalize)
            .with_status(&LifecycleStatus::Complete)
            .build()
            .unwrap();

        store.add_service(service.clone()).unwrap();

        interface
            .retire_service(
                service.service_id().circuit_id().as_str(),
                service.service_id().service_id().as_str(),
                &service.service_type().to_string(),
            )
            .unwrap();

        let service = store
            .get_service(&service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        assert_eq!(service.command(), &LifecycleCommand::Retire);
        assert_eq!(service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    // Verify that the SyncLifecycleInterface will properly wait for a service to be purged
    // before returning
    //
    // 1. Setup the LifecycleExecutor
    // 2. Add a retired service to the LifecycleStore
    // 2. Call purge_service and verify it returns Ok
    // 3. Check that an associated service has been removed from the LifecycleStore
    #[test]
    fn test_purge_service() {
        let (mut executor, store) = create_executor(Duration::from_secs(10));

        let alarm = executor.alarm();
        let interface = SyncLifecycleInterface::new(
            store.clone_box(),
            alarm,
            vec!["test".to_string()],
            Duration::from_secs(10),
        );

        let service = create_service();

        let service = service
            .into_builder()
            .with_command(&LifecycleCommand::Retire)
            .with_status(&LifecycleStatus::Complete)
            .build()
            .unwrap();

        store.add_service(service.clone()).unwrap();

        interface
            .purge_service(
                service.service_id().circuit_id().as_str(),
                service.service_id().service_id().as_str(),
                &service.service_type().to_string(),
            )
            .unwrap();

        assert!(store
            .get_service(&service.service_id())
            .expect("unable to get service")
            .is_none());

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }
}
