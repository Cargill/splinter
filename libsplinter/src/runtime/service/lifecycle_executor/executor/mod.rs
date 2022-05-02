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

//! This module contains an Executor for running lifecycles

mod alarm;
mod message;
mod thread;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use crate::error::InternalError;
use crate::runtime::service::{LifecycleCommandGenerator, LifecycleStore};
use crate::service::{Lifecycle, ServiceType};
use crate::store::command::StoreCommandExecutor;
use crate::threading::{lifecycle::ShutdownHandle, pacemaker::Pacemaker};

use self::message::ExecutorMessage;
use self::thread::ExecutorThread;

pub use self::alarm::{ChannelExecutorAlarm, ExecutorAlarm};

pub struct LifecycleExecutor<E: 'static>
where
    E: StoreCommandExecutor + Send,
{
    pacemaker: Pacemaker,
    sender: Sender<ExecutorMessage>,
    executor_thread: ExecutorThread<E>,
    _executor: PhantomData<E>,
}

type LifecycleMap<E> =
    HashMap<ServiceType<'static>, Box<dyn Lifecycle<E, Arguments = Vec<(String, String)>> + Send>>;

impl<E: 'static> LifecycleExecutor<E>
where
    E: StoreCommandExecutor + Send,
{
    pub fn new(
        wake_up_interval: Duration,
        lifecycles: LifecycleMap<E::Context>,
        store: Box<dyn LifecycleStore + Send>,
        command_generator: LifecycleCommandGenerator<E::Context>,
        command_executor: E,
    ) -> Result<LifecycleExecutor<E>, InternalError> {
        let (sender, recv) = channel();
        let pacemaker = Pacemaker::builder()
            .with_interval(wake_up_interval.as_secs())
            .with_sender(sender.clone())
            .with_message_factory(|| ExecutorMessage::WakeUpAll)
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let executor_thread =
            ExecutorThread::start(recv, lifecycles, store, command_generator, command_executor)?;

        Ok(LifecycleExecutor {
            pacemaker,
            sender,
            executor_thread,
            _executor: PhantomData,
        })
    }

    /// Get a `ExecutorAlarm` that can be use to prematurely wake up the `LifecycleExecutor`
    pub fn alarm(&self) -> Box<dyn ExecutorAlarm> {
        Box::new(ChannelExecutorAlarm::new(self.sender.clone()))
    }
}

impl<E> ShutdownHandle for LifecycleExecutor<E>
where
    E: StoreCommandExecutor + Send,
{
    fn signal_shutdown(&mut self) {
        self.pacemaker.shutdown_signaler().shutdown();
        if self.sender.send(ExecutorMessage::Shutdown).is_err() {
            warn!("Lifecycle executor is no longer running");
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        debug!("Shutting down lifecycle executor...");
        self.executor_thread.join()?;
        debug!("Shutting down lifecycle executor(complete)");
        Ok(())
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::{mpsc::Receiver, Arc};

    use diesel::r2d2::{ConnectionManager, Pool};

    use crate::migrations::run_sqlite_migrations;
    use crate::runtime::service::lifecycle_executor::store::diesel::factory::SqliteLifecycleStoreFactory;
    use crate::runtime::service::{
        DieselLifecycleStore, LifecycleCommand, LifecycleService, LifecycleServiceBuilder,
        LifecycleStatus, LifecycleStore, LifecycleStoreFactory,
    };
    use crate::service::FullyQualifiedServiceId;
    use crate::store::command::{DieselStoreCommandExecutor, StoreCommand};

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
        message: String,
        sender: Sender<String>,
        _context: PhantomData<C>,
    }

    impl<C> StoreCommand for TestCommand<C> {
        type Context = C;

        fn execute(&self, _conn: &Self::Context) -> Result<(), InternalError> {
            self.sender
                .send(self.message.clone())
                .map_err(|err| InternalError::from_source(Box::new(err)))
        }
    }

    struct TestLifecycle<K: 'static> {
        sender: Sender<String>,
        _context: PhantomData<K>,
    }

    impl<K: 'static> TestLifecycle<K> {
        fn new(sender: Sender<String>) -> Self {
            TestLifecycle {
                sender,
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
            Ok(Box::new(TestCommand {
                message: "Prepare".to_string(),
                sender: self.sender.clone(),
                _context: PhantomData,
            }))
        }

        fn command_to_finalize(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand {
                message: "Finalize".to_string(),
                sender: self.sender.clone(),
                _context: PhantomData,
            }))
        }

        fn command_to_retire(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand {
                message: "Retire".to_string(),
                sender: self.sender.clone(),
                _context: PhantomData,
            }))
        }

        fn command_to_purge(
            &self,
            _service: FullyQualifiedServiceId,
        ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
            Ok(Box::new(TestCommand {
                message: "Purge".to_string(),
                sender: self.sender.clone(),
                _context: PhantomData,
            }))
        }
    }

    fn create_executor(
        wake_up_interval: Duration,
    ) -> (
        LifecycleExecutor<DieselStoreCommandExecutor<diesel::sqlite::SqliteConnection>>,
        Box<dyn LifecycleStore>,
        Receiver<String>,
    ) {
        let pool = create_connection_pool_and_migrate();

        let mut lifecycles: HashMap<
            ServiceType<'static>,
            Box<
                dyn Lifecycle<diesel::sqlite::SqliteConnection, Arguments = Vec<(String, String)>>
                    + Send,
            >,
        > = HashMap::new();

        let (sender, recv) = channel();

        let test_lifecycle = TestLifecycle::new(sender);

        lifecycles.insert(ServiceType::new("test").unwrap(), Box::new(test_lifecycle));

        let store = Box::new(DieselLifecycleStore::new(pool.clone()));
        let store_factory: Arc<(dyn LifecycleStoreFactory<diesel::sqlite::SqliteConnection>)> =
            Arc::new(SqliteLifecycleStoreFactory);

        let command_generator = LifecycleCommandGenerator::new(store_factory);
        let command_executor = DieselStoreCommandExecutor::new(pool.clone());

        let executor = LifecycleExecutor::new(
            wake_up_interval,
            lifecycles,
            store.clone(),
            command_generator,
            command_executor,
        )
        .unwrap();

        (executor, store, recv)
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be prepared.
    ///
    /// 1. Start the executor
    /// 2. Add Service with status New and command Prepare
    /// 3. Wait for the pacemaker to wake the lifecycle executor
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_all_prepare() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(1));
        let service = create_service();

        store.add_service(service.clone()).unwrap();
        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Prepare".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be finalized.
    ///
    /// 1. Start the executor
    /// 2. Add Service with status New and command Prepare
    /// 3. Wait for the pacemaker to wake the lifecycle executor
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_all_finalized() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(1));
        let service = create_service();

        let service = service
            .into_builder()
            .with_command(&LifecycleCommand::Finalize)
            .build()
            .unwrap();

        store.add_service(service.clone()).unwrap();
        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Finalize".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be retired.
    ///
    /// 1. Start the executor
    /// 2. Add Service with status New and command Retire
    /// 3. Wait for the pacemaker to wake the lifecycle executor
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_all_retire() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(1));
        let service = create_service();

        let service = service
            .into_builder()
            .with_command(&LifecycleCommand::Retire)
            .build()
            .unwrap();

        store.add_service(service.clone()).unwrap();
        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Retire".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be purged.
    ///
    /// 1. Start the executor
    /// 2. Add Service with status New and command Purged
    /// 3. Wait for the pacemaker to wake the lifecycle executor
    /// 4. Verify that the Service has been removed
    #[test]
    fn test_executor_wake_up_all_purge() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(1));
        let service = create_service();

        let service = service
            .into_builder()
            .with_command(&LifecycleCommand::Purge)
            .build()
            .unwrap();

        store.add_service(service.clone()).unwrap();
        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Purge".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.is_none() {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
        }

        assert!(fetched_service.is_none());

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be prepared by an alarm.
    ///
    /// 1. Start the executor with a large wake up interval so it will not trigger
    /// 2. Add Service with status New and command Prepare
    /// 3. Use the alarm to wake up all
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_all_alarm_prepare() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(500));
        let alarm = executor.alarm();

        let service = create_service();
        store.add_service(service.clone()).unwrap();

        alarm.wake_up_all().unwrap();

        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Prepare".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be prepared by an alarm.
    ///
    /// 1. Start the executor with a large wake up interval so it will not trigger
    /// 2. Add Service with status New and command Prepare
    /// 3. Use the alarm to wake up "test" service type with no service id
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_service_type_prepare() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(500));
        let alarm = executor.alarm();

        let service = create_service();
        store.add_service(service.clone()).unwrap();

        alarm
            .wake_up(ServiceType::new("test").unwrap(), None)
            .unwrap();

        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Prepare".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be prepared by an alarm.
    ///
    /// 1. Start the executor with a large wake up interval so it will not trigger
    /// 2. Add Service with status New and command Prepare
    /// 3. Use the alarm to wake up "test" service with specific service id
    /// 4. Verify that the Service has been updated
    #[test]
    fn test_executor_wake_up_service_id_prepare() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(500));
        let alarm = executor.alarm();

        let service = create_service();
        store.add_service(service.clone()).unwrap();

        alarm
            .wake_up(
                ServiceType::new("test").unwrap(),
                Some(service.service_id().clone()),
            )
            .unwrap();

        if let Ok(msg) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            assert_eq!(msg, "Prepare".to_string())
        } else {
            panic!("Test timed out, lifecycle did not wake up to handle pending service")
        }

        let mut fetched_service = store
            .get_service(service.service_id())
            .expect("unable to get service")
            .expect("service was none");

        // there is a chance the message will be sent before the store is finished updating,
        // this is not an issue when only writing to the database
        for _ in 1..5 {
            if fetched_service.status() == &LifecycleStatus::Complete {
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
            fetched_service = store
                .get_service(service.service_id())
                .expect("unable to get service")
                .expect("service was none");
        }

        assert_eq!(fetched_service.status(), &LifecycleStatus::Complete);

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }

    /// Test that the lifecycle executor will properly wake up and handle a service that needs to
    /// be prepared by an alarm.
    ///
    /// 1. Start the executor with a large wake up interval so it will not trigger
    /// 2. Add Service with status New and command Prepare
    /// 3. Use the alarm to wake up "test" services with a bad service id
    /// 4. Verify that the executor does not run the test lifecycle
    #[test]
    fn test_executor_wake_up_bad_service_id_prepare() {
        let (mut executor, store, recv) = create_executor(Duration::from_secs(500));
        let alarm = executor.alarm();

        let service = create_service();
        store.add_service(service.clone()).unwrap();

        alarm
            .wake_up(
                ServiceType::new("test").unwrap(),
                Some(FullyQualifiedServiceId::new_random()),
            )
            .unwrap();

        if let Ok(_) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
            panic!("Should not have recieved a message")
        }

        executor.signal_shutdown();
        executor.wait_for_shutdown().unwrap();
    }
}
