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

mod models;
mod operations;
mod pool;
mod schema;

use std::sync::{Arc, RwLock};

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{
    connection::AnsiTransactionManager,
    r2d2::{ConnectionManager, Pool},
    Connection,
};
use splinter::{error::InternalError, service::FullyQualifiedServiceId, service::ServiceId};

use pool::ConnectionPool;

use crate::service::EchoArguments;
use crate::service::EchoRequest;
use crate::service::EchoServiceStatus;
use crate::service::RequestStatus;

use super::EchoStore;

use operations::add_service::AddServiceOperation as _;
use operations::get_last_sent::GetLastSentOperation as _;
use operations::get_service_arguments::GetServiceArgumentsOperation as _;
use operations::get_service_status::GetServiceStatusOperation as _;
use operations::insert_request::InsertRequestOperation as _;
use operations::insert_request_error::InsertRequestErrorOperation as _;
use operations::list_ready_services::ListReadyServicesOperation as _;
use operations::list_requests::ListRequestsOperation as _;
use operations::remove_service::RemoveServiceOperation as _;
use operations::update_request_ack::UpdateRequestAckOperation as _;
use operations::update_request_sent::UpdateRequestSentOperation as _;
use operations::update_service_status::UpdateServiceStatusOperation as _;
use operations::EchoStoreOperations;

pub struct DieselEchoStore<C: Connection + 'static> {
    pool: ConnectionPool<C>,
}

impl<C: Connection> DieselEchoStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self {
            pool: connection_pool.into(),
        }
    }

    pub fn new_with_write_exclusivity(
        connection_pool: Arc<RwLock<Pool<ConnectionManager<C>>>>,
    ) -> Self {
        Self {
            pool: connection_pool.into(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl EchoStore for DieselEchoStore<SqliteConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).add_service(service, arguments))
    }

    fn remove_service(&self, service: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).remove_service(service))
    }

    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError> {
        self.pool
            .execute_read(|conn| EchoStoreOperations::new(conn).get_service_arguments(service))
    }

    fn insert_request(
        &self,
        service: &FullyQualifiedServiceId,
        to_service: &ServiceId,
        message: &str,
    ) -> Result<u64, InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).insert_request(service, to_service, message)
        })
    }

    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_request_sent(
                service,
                correlation_id,
                sent,
                sent_at,
            )
        })
    }

    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError> {
        self.pool.execute_read(|conn| {
            EchoStoreOperations::new(conn).get_last_sent(sender_service_id, receiver_service_id)
        })
    }

    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError> {
        self.pool.execute_read(|conn| {
            EchoStoreOperations::new(conn).list_requests(service, receiver_service_id)
        })
    }

    fn update_request_ack(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        ack: RequestStatus,
        ack_at: Option<i64>,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_request_ack(service, correlation_id, ack, ack_at)
        })
    }

    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).insert_request_error(service, error_message, error_at)
        })
    }

    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).list_ready_services())
    }

    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_service_status(service, status)
        })
    }

    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError> {
        self.pool
            .execute_read(|conn| EchoStoreOperations::new(conn).get_service_status(service))
    }
}

#[cfg(feature = "postgres")]
impl EchoStore for DieselEchoStore<PgConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).add_service(service, arguments))
    }

    fn remove_service(&self, service: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).remove_service(service))
    }

    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError> {
        self.pool
            .execute_read(|conn| EchoStoreOperations::new(conn).get_service_arguments(service))
    }

    fn insert_request(
        &self,
        service: &FullyQualifiedServiceId,
        to_service: &ServiceId,
        message: &str,
    ) -> Result<u64, InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).insert_request(service, to_service, message)
        })
    }

    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_request_sent(
                service,
                correlation_id,
                sent,
                sent_at,
            )
        })
    }

    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError> {
        self.pool.execute_read(|conn| {
            EchoStoreOperations::new(conn).get_last_sent(sender_service_id, receiver_service_id)
        })
    }

    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError> {
        self.pool.execute_read(|conn| {
            EchoStoreOperations::new(conn).list_requests(service, receiver_service_id)
        })
    }

    fn update_request_ack(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        ack: RequestStatus,
        ack_at: Option<i64>,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_request_ack(service, correlation_id, ack, ack_at)
        })
    }

    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).insert_request_error(service, error_message, error_at)
        })
    }

    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        self.pool
            .execute_write(|conn| EchoStoreOperations::new(conn).list_ready_services())
    }

    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError> {
        self.pool.execute_write(|conn| {
            EchoStoreOperations::new(conn).update_service_status(service, status)
        })
    }

    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError> {
        self.pool
            .execute_read(|conn| EchoStoreOperations::new(conn).get_service_status(service))
    }
}

pub struct DieselConnectionEchoStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    connection: &'a C,
}

impl<'a, C> DieselConnectionEchoStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    pub fn new(connection: &'a C) -> Self {
        DieselConnectionEchoStore { connection }
    }
}

#[cfg(feature = "sqlite")]
impl<'a> EchoStore for DieselConnectionEchoStore<'a, SqliteConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).add_service(service, arguments)
    }

    fn remove_service(&self, service: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).remove_service(service)
    }

    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError> {
        EchoStoreOperations::new(self.connection).get_service_arguments(service)
    }

    fn insert_request(
        &self,
        service: &FullyQualifiedServiceId,
        to_service: &ServiceId,
        message: &str,
    ) -> Result<u64, InternalError> {
        EchoStoreOperations::new(self.connection).insert_request(service, to_service, message)
    }

    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_request_sent(
            service,
            correlation_id,
            sent,
            sent_at,
        )
    }

    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError> {
        EchoStoreOperations::new(self.connection)
            .get_last_sent(sender_service_id, receiver_service_id)
    }

    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError> {
        EchoStoreOperations::new(self.connection).list_requests(service, receiver_service_id)
    }

    fn update_request_ack(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        ack: RequestStatus,
        ack_at: Option<i64>,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_request_ack(
            service,
            correlation_id,
            ack,
            ack_at,
        )
    }

    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
        EchoStoreOperations::new(self.connection).insert_request_error(
            service,
            error_message,
            error_at,
        )
    }

    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        EchoStoreOperations::new(self.connection).list_ready_services()
    }

    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_service_status(service, status)
    }

    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError> {
        EchoStoreOperations::new(self.connection).get_service_status(service)
    }
}

#[cfg(feature = "postgres")]
impl<'a> EchoStore for DieselConnectionEchoStore<'a, PgConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).add_service(service, arguments)
    }

    fn remove_service(&self, service: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).remove_service(service)
    }

    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError> {
        EchoStoreOperations::new(self.connection).get_service_arguments(service)
    }

    fn insert_request(
        &self,
        service: &FullyQualifiedServiceId,
        to_service: &ServiceId,
        message: &str,
    ) -> Result<u64, InternalError> {
        EchoStoreOperations::new(self.connection).insert_request(service, to_service, message)
    }

    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_request_sent(
            service,
            correlation_id,
            sent,
            sent_at,
        )
    }

    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError> {
        EchoStoreOperations::new(self.connection)
            .get_last_sent(sender_service_id, receiver_service_id)
    }

    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError> {
        EchoStoreOperations::new(self.connection).list_requests(service, receiver_service_id)
    }

    fn update_request_ack(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        ack: RequestStatus,
        ack_at: Option<i64>,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_request_ack(
            service,
            correlation_id,
            ack,
            ack_at,
        )
    }

    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
        EchoStoreOperations::new(self.connection).insert_request_error(
            service,
            error_message,
            error_at,
        )
    }

    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        EchoStoreOperations::new(self.connection).list_ready_services()
    }

    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError> {
        EchoStoreOperations::new(self.connection).update_service_status(service, status)
    }

    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError> {
        EchoStoreOperations::new(self.connection).get_service_status(service)
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use std::convert::TryFrom;
    use std::time::SystemTime;

    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use splinter::service::ServiceId;

    #[test]
    fn echo_store_sqlite_add_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        assert!(store.add_service(&fqsi, &echo_args).is_ok());
        // adding service with same ID should fail
        assert!(store.add_service(&fqsi, &echo_args).is_err());
    }

    #[test]
    fn echo_store_sqlite_remove_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        assert!(store.remove_service(&fqsi).is_err());

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add echo service");

        assert!(store.remove_service(&fqsi).is_ok());
    }

    #[test]
    fn echo_store_sqlite_get_service_args() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service1 = ServiceId::new_random();
        let peer_service2 = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service1.clone(), peer_service2.clone()],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        assert!(store.get_service_arguments(&fqsi).is_err());

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add echo service");

        let service_args = store
            .get_service_arguments(&fqsi)
            .expect("failed to get service args");

        assert!(service_args.peers().contains(&peer_service1));
        assert!(service_args.peers().contains(&peer_service2));
        assert_eq!(service_args.frequency(), &std::time::Duration::from_secs(2));
        assert_eq!(service_args.jitter(), &std::time::Duration::from_secs(1));
        assert_eq!(service_args.error_rate(), 0.5);

        // test that a service with no peers can be added and retrieved

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add echo service");

        let service_args = store
            .get_service_arguments(&fqsi2)
            .expect("failed to get service args");

        assert_eq!(service_args.peers(), &vec![]);
        assert_eq!(service_args.frequency(), &std::time::Duration::from_secs(2));
        assert_eq!(service_args.jitter(), &std::time::Duration::from_secs(1));
        assert_eq!(service_args.error_rate(), 0.5);
    }

    #[test]
    fn echo_store_sqlite_insert_request() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        assert!(store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .is_ok());
    }

    #[test]
    fn echo_store_sqlite_update_request_sent() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        let correlation_id = store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .expect("failed to insert request");

        let sent_at = i64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("failed to get sent_at time")
                .as_secs(),
        )
        .expect("failed to convert u64 to i64");

        assert!(store
            .update_request_sent(
                &fqsi2,
                correlation_id as i64,
                RequestStatus::Sent,
                Some(sent_at)
            )
            .is_ok());
        assert!(store
            .update_request_sent(&fqsi2, 99999999, RequestStatus::Sent, None)
            .is_err());
    }

    #[test]
    fn echo_store_sqlite_get_last_request() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        let correlation_id1 = store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .expect("failed to insert request");

        let correlation_id2 = store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .expect("failed to insert request");

        let sent_at1 = i64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("failed to get sent_at time")
                .as_secs(),
        )
        .expect("failed to convert u64 to i64");

        let sent_at2 = i64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("failed to get sent_at time")
                .as_secs(),
        )
        .expect("failed to convert u64 to i64");

        store
            .update_request_sent(
                &fqsi2,
                correlation_id1 as i64,
                RequestStatus::Sent,
                Some(sent_at1),
            )
            .expect("failed to update request");

        store
            .update_request_sent(
                &fqsi2,
                correlation_id2 as i64,
                RequestStatus::Sent,
                Some(sent_at2),
            )
            .expect("failed to update request");

        let last_sent = store
            .get_last_sent(&fqsi2, fqsi.service_id())
            .expect("failed to get last sent");

        assert_eq!(last_sent, Some(sent_at2));
    }

    #[test]
    fn echo_store_sqlite_list_requests() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        let correlation_id1 = store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .expect("failed to insert request");

        let requests = store
            .list_requests(&fqsi2, Some(fqsi.service_id()))
            .expect("failed to list unsent");

        assert_eq!(requests[0].correlation_id, correlation_id1 as i64);
        assert_eq!(requests[0].message, "test".to_string());
        assert_eq!(&requests[0].receiver_service_id, fqsi.service_id());
    }

    #[test]
    fn echo_store_sqlite_update_request_ack() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        let correlation_id = store
            .insert_request(&fqsi2, fqsi.service_id(), "test")
            .expect("failed to insert request");

        let ack_at = i64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("failed to get ack_at time")
                .as_secs(),
        )
        .expect("failed to convert u64 to i64");

        assert!(store
            .update_request_ack(
                &fqsi2,
                correlation_id as i64,
                RequestStatus::Sent,
                Some(ack_at)
            )
            .is_ok());
        assert!(store
            .update_request_ack(&fqsi2, 99999999, RequestStatus::Sent, None)
            .is_err());
    }

    #[test]
    fn echo_store_sqlite_insert_request_error() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let echo_args = EchoArguments::new(
            vec![],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add first echo service");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let echo_args2 = EchoArguments::new(
            vec![ServiceId::new("abcde").expect("failed to get service ID")],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(1),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi2, &echo_args2)
            .expect("failed to add second echo service");

        let error_at = i64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("failed to get ack_at time")
                .as_secs(),
        )
        .expect("failed to convert u64 to i64");

        assert!(store
            .insert_request_error(&fqsi2, "test_error", error_at)
            .is_ok());
    }

    #[test]
    fn echo_store_sqlite_list_ready_services() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("fghij-abcde::bb00")
            .expect("creating FullyQualifiedServiceId from string 'fghij-abcde::bb00'");

        let peer_service = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        let echo_args2 = EchoArguments::new(
            vec![],
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

        let service_ids = store
            .list_ready_services()
            .expect("failed to list ready service IDs");

        assert_eq!(vec![fqsi], service_ids);
    }

    #[test]
    fn echo_store_sqlite_update_service_status() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add service");

        assert!(store
            .update_service_status(&fqsi, EchoServiceStatus::Prepared)
            .is_ok());
    }

    #[test]
    fn echo_store_sqlite_get_service_status() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselEchoStore::new(pool);

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service = ServiceId::new_random();

        let echo_args = EchoArguments::new(
            vec![peer_service],
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2),
            0.5,
        )
        .expect("failed to create echo arguments");

        store
            .add_service(&fqsi, &echo_args)
            .expect("failed to add service");

        let status = store
            .get_service_status(&fqsi)
            .expect("failed to get status");

        assert_eq!(status, EchoServiceStatus::Prepared)
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
