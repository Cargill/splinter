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

use splinter::error::InternalError;
use splinter::service::{FullyQualifiedServiceId, Routable, ServiceType, TimerFilter};

use crate::store::PooledEchoStoreFactory;

const STATIC_TYPES: &[ServiceType] = &[ServiceType::new_static("echo")];

// Used to determine the list of service ids that need to be handled. after calling this, the code
// will call TimerHandler for each.
pub struct EchoTimerFilter {
    store_factory: Box<dyn PooledEchoStoreFactory>,
}

impl EchoTimerFilter {
    pub fn new(store_factory: Box<dyn PooledEchoStoreFactory>) -> Self {
        Self { store_factory }
    }
}

impl TimerFilter for EchoTimerFilter {
    // get the service IDs of all services which need to be handled
    fn filter(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        self.store_factory.new_store().list_ready_services()
    }
}

impl Routable for EchoTimerFilter {
    fn service_types(&self) -> &[ServiceType] {
        STATIC_TYPES
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, RwLock};

    use crate::migrations::run_sqlite_migrations;
    use crate::service::EchoArguments;
    use crate::service::EchoServiceStatus;
    use crate::store::PooledSqliteEchoStoreFactory;
    use crate::store::{DieselEchoStore, EchoStore};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use splinter::service::ServiceId;

    #[test]
    fn test_echo_timer_filter() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool.clone());

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("abcde-fghij::bb00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::bb00'");

        let peer_service1 =
            ServiceId::new(String::from("bb00")).expect("failed to make service ID aa00");
        let peer_service2 =
            ServiceId::new(String::from("aa00")).expect("failed to make service ID bb00");

        let echo_args = EchoArguments::new(
            vec![peer_service1],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        let echo_args2 = EchoArguments::new(
            vec![peer_service2],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add service");
        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add service");

        store
            .update_service_status(&fqsi, EchoServiceStatus::Finalized)
            .expect("failed to update service status to finalized");

        let echo_timer_filter = EchoTimerFilter::new(Box::new(
            PooledSqliteEchoStoreFactory::new_with_write_exclusivity(Arc::new(RwLock::new(pool))),
        ));

        let ids = echo_timer_filter.filter().expect("failed to filter");

        assert_eq!(vec![fqsi], ids);
    }

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
