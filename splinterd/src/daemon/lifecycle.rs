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

use splinter::error::InternalError;
#[cfg(feature = "database-postgres")]
use splinter::runtime::service::PostgresLifecycleStoreFactory;
#[cfg(feature = "database-sqlite")]
use splinter::runtime::service::SqliteLifecycleStoreFactory;
use splinter::runtime::service::{
    ExecutorAlarm, LifecycleCommandGenerator, LifecycleExecutor, LifecycleStore,
    LifecycleStoreFactory,
};
use splinter::store::command::DieselStoreCommandExecutor;
use splinter::threading::lifecycle::ShutdownHandle;

use super::store::ConnectionPool;

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
            let lifecycles = HashMap::new();
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
            let lifecycles = HashMap::new();
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
