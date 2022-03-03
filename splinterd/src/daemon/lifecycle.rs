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
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "scabbardv3")]
use scabbard::service::v3::{ScabbardArgumentsVecConverter, ScabbardLifecycle};
#[cfg(all(feature = "scabbardv3", feature = "database-postgres"))]
use scabbard::store::PgScabbardStoreFactory;
#[cfg(all(feature = "scabbardv3", feature = "database-sqlite"))]
use scabbard::store::SqliteScabbardStoreFactory;
use splinter::error::InternalError;
#[cfg(feature = "database-postgres")]
use splinter::runtime::service::PostgresLifecycleStoreFactory;
#[cfg(feature = "database-sqlite")]
use splinter::runtime::service::SqliteLifecycleStoreFactory;
use splinter::runtime::service::{
    ExecutorAlarm, LifecycleCommandGenerator, LifecycleExecutor, LifecycleStore,
    LifecycleStoreFactory,
};
#[cfg(any(feature = "scabbardv3", feature = "service-echo"))]
use splinter::service::{Lifecycle, ServiceType};
use splinter::store::command::DieselStoreCommandExecutor;
use splinter::threading::lifecycle::ShutdownHandle;
#[cfg(feature = "service-echo")]
use splinter_echo::service::{EchoArgumentsVecConverter, EchoLifecycle};
#[cfg(all(feature = "service-echo", feature = "database-postgres"))]
use splinter_echo::store::PgEchoStoreFactory;
#[cfg(all(feature = "service-echo", feature = "database-sqlite"))]
use splinter_echo::store::SqliteEchoStoreFactory;

use super::store::ConnectionPool;
#[cfg(feature = "service-echo")]
use super::ECHO_SERVICE_TYPE;
use super::SCABBARD_SERVICE_TYPE;

pub enum DaemonLifecycleExecutor {
    #[cfg(feature = "database-sqlite")]
    Sqlite {
        executor: LifecycleExecutor<DieselStoreCommandExecutor<diesel::sqlite::SqliteConnection>>,
    },
    #[cfg(feature = "database-postgres")]
    Postgres {
        executor: LifecycleExecutor<DieselStoreCommandExecutor<diesel::pg::PgConnection>>,
    },
}

impl DaemonLifecycleExecutor {
    /// Get a `ExecutorAlarm` that can be use to prematurely wake up the `LifecycleExecutor`
    pub fn alarm(&self) -> Box<dyn ExecutorAlarm> {
        match self {
            #[cfg(feature = "database-sqlite")]
            DaemonLifecycleExecutor::Sqlite { executor } => executor.alarm(),
            #[cfg(feature = "database-postgres")]
            DaemonLifecycleExecutor::Postgres { executor } => executor.alarm(),
        }
    }
}

impl ShutdownHandle for DaemonLifecycleExecutor {
    fn signal_shutdown(&mut self) {
        match self {
            #[cfg(feature = "database-sqlite")]
            DaemonLifecycleExecutor::Sqlite { executor } => executor.signal_shutdown(),
            #[cfg(feature = "database-postgres")]
            DaemonLifecycleExecutor::Postgres { executor } => executor.signal_shutdown(),
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        match self {
            #[cfg(feature = "database-sqlite")]
            DaemonLifecycleExecutor::Sqlite { executor } => executor.wait_for_shutdown(),
            #[cfg(feature = "database-postgres")]
            DaemonLifecycleExecutor::Postgres { executor } => executor.wait_for_shutdown(),
        }
    }
}

pub fn create_lifecycle_executor(
    connection_pool: &ConnectionPool,
    lifecycle_store: Box<dyn LifecycleStore + Send>,
    lifecycle_executor_interval: Duration,
) -> Result<DaemonLifecycleExecutor, InternalError> {
    match connection_pool {
        #[cfg(feature = "database-sqlite")]
        ConnectionPool::Sqlite { pool } => {
            #[cfg_attr(
                not(any(feature = "scabbardv3", feature = "service-echo")),
                allow(usused_mut)
            )]
            let mut lifecycles: SqliteLifecycles = HashMap::new();

            #[cfg(feature = "scabbardv3")]
            {
                let scabbard_lifecycle =
                    ScabbardLifecycle::new(Arc::new(SqliteScabbardStoreFactory));
                let scabbard_vec_lifecycle =
                    scabbard_lifecycle.into_lifecycle(ScabbardArgumentsVecConverter {});
                lifecycles.insert(SCABBARD_SERVICE_TYPE, Box::new(scabbard_vec_lifecycle));
            }

            #[cfg(feature = "service-echo")]
            {
                let echo_lifecycle = EchoLifecycle::new(Arc::new(SqliteEchoStoreFactory));
                let echo_vec_lifecycle =
                    echo_lifecycle.into_lifecycle(EchoArgumentsVecConverter {});
                lifecycles.insert(ECHO_SERVICE_TYPE, Box::new(echo_vec_lifecycle));
            }

            let lifecycle_pool = pool.write().unwrap().clone();
            let lifecycle_store_factory: Arc<
                (dyn LifecycleStoreFactory<diesel::sqlite::SqliteConnection>),
            > = Arc::new(SqliteLifecycleStoreFactory);

            let command_generator = LifecycleCommandGenerator::new(lifecycle_store_factory);
            let command_executor = DieselStoreCommandExecutor::new(lifecycle_pool);
            let executor = LifecycleExecutor::new(
                lifecycle_executor_interval,
                lifecycles,
                lifecycle_store,
                command_generator,
                command_executor,
            )?;

            Ok(DaemonLifecycleExecutor::Sqlite { executor })
        }
        #[cfg(feature = "database-postgres")]
        ConnectionPool::Postgres { pool } => {
            #[cfg_attr(
                not(any(feature = "scabbardv3", feature = "service-echo")),
                allow(usused_mut)
            )]
            let mut lifecycles: PostgresLifecycles = HashMap::new();

            #[cfg(feature = "scabbardv3")]
            {
                let scabbard_lifecycle = ScabbardLifecycle::new(Arc::new(PgScabbardStoreFactory));
                let scabbard_vec_lifecycle =
                    scabbard_lifecycle.into_lifecycle(ScabbardArgumentsVecConverter {});
                lifecycles.insert(SCABBARD_SERVICE_TYPE, Box::new(scabbard_vec_lifecycle));
            }

            #[cfg(feature = "service-echo")]
            {
                let echo_lifecycle = EchoLifecycle::new(Arc::new(PgEchoStoreFactory));
                let echo_vec_lifecycle =
                    echo_lifecycle.into_lifecycle(EchoArgumentsVecConverter {});
                lifecycles.insert(ECHO_SERVICE_TYPE, Box::new(echo_vec_lifecycle));
            }

            let lifecycle_pool = pool.clone();
            let lifecycle_store_factory: Arc<
                (dyn LifecycleStoreFactory<diesel::pg::PgConnection>),
            > = Arc::new(PostgresLifecycleStoreFactory);

            let command_generator = LifecycleCommandGenerator::new(lifecycle_store_factory);
            let command_executor = DieselStoreCommandExecutor::new(lifecycle_pool);
            let executor = LifecycleExecutor::new(
                lifecycle_executor_interval,
                lifecycles,
                lifecycle_store,
                command_generator,
                command_executor,
            )?;

            Ok(DaemonLifecycleExecutor::Postgres { executor })
        }
        #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
        ConnectionPool::Unsupported => Err(InternalError::with_message(
            "Connection pools are unavailable in this configuration".into(),
        )),
    }
}

#[cfg(feature = "database-sqlite")]
type SqliteLifecycles = HashMap<
    ServiceType<'static>,
    Box<dyn Lifecycle<diesel::sqlite::SqliteConnection, Arguments = Vec<(String, String)>> + Send>,
>;

#[cfg(feature = "database-postgres")]
type PostgresLifecycles = HashMap<
    ServiceType<'static>,
    Box<dyn Lifecycle<diesel::pg::PgConnection, Arguments = Vec<(String, String)>> + Send>,
>;
