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

#[cfg(feature = "scabbardv3")]
use std::sync::Arc;

#[cfg(feature = "scabbardv3")]
use scabbard::service::v3::{ScabbardTimerFilter, ScabbardTimerHandlerFactoryBuilder};
#[cfg(all(feature = "scabbardv3", feature = "database-postgres"))]
use scabbard::store::PgScabbardStoreFactory;
#[cfg(all(feature = "scabbardv3", feature = "database-sqlite"))]
use scabbard::store::SqliteScabbardStoreFactory;
#[cfg(feature = "scabbardv3")]
use splinter::circuit::routing::RoutingTableReader;
use splinter::error::InternalError;
#[cfg(feature = "scabbardv3")]
use splinter::peer::interconnect::NetworkMessageSender;
#[cfg(feature = "scabbardv3")]
use splinter::runtime::service::NetworkMessageSenderFactory;
use splinter::service::{TimerFilter, TimerHandlerFactory};
#[cfg(feature = "scabbardv3")]
use splinter::store::command::DieselStoreCommandExecutor;

use super::store::ConnectionPool;

#[cfg(feature = "service2")]
type TimerFilterCollection = Vec<(
    Box<dyn TimerFilter + Send>,
    Box<dyn TimerHandlerFactory<Message = Vec<u8>>>,
)>;

pub fn create_timer_handlers(
    connection_pool: &ConnectionPool,
    node_id: &str,
    network_sender: NetworkMessageSender,
    routing_reader: Box<dyn RoutingTableReader>,
) -> Result<TimerFilterCollection, InternalError> {
    #[cfg_attr(not(feature = "scabbardv3"), allow(clippy::redundant_clone))]
    let mut timer_filter_collection: TimerFilterCollection = vec![];

    #[cfg(feature = "scabbardv3")]
    {
        match connection_pool {
            #[cfg(feature = "database-postgres")]
            ConnectionPool::Postgres { pool } => {
                let mut timer_scabbard_factory_builder = ScabbardTimerHandlerFactoryBuilder::new()
                    .with_message_sender_factory(Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender,
                        routing_reader,
                    )));

                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_pooled_store_factory(Box::new(
                        scabbard::store::PooledPgScabbardStoreFactory::new(pool.clone()),
                    ));

                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_store_factory(Arc::new(PgScabbardStoreFactory));
                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_store_command_executor(Arc::new(DieselStoreCommandExecutor::new(
                        pool.clone(),
                    )));
                let timer_scabbard_factory: Box<dyn TimerHandlerFactory<Message = Vec<u8>>> =
                    Box::new(
                        timer_scabbard_factory_builder
                            .build()
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                    );

                let scabbard_timer_filter = ScabbardTimerFilter::new(Box::new(
                    scabbard::store::PooledPgScabbardStoreFactory::new(pool.clone()),
                ));
                timer_filter_collection
                    .push((Box::new(scabbard_timer_filter), timer_scabbard_factory));

                Ok(timer_filter_collection)
            }
            #[cfg(feature = "database-sqlite")]
            ConnectionPool::Sqlite { pool } => {
                let mut timer_scabbard_factory_builder = ScabbardTimerHandlerFactoryBuilder::new()
                    .with_message_sender_factory(Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender,
                        routing_reader,
                    )));
                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_pooled_store_factory(Box::new(
                    scabbard::store::PooledSqliteScabbardStoreFactory::new_with_write_exclusivity(
                        pool.clone(),
                    ),
                ));
                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_store_factory(Arc::new(SqliteScabbardStoreFactory));
                timer_scabbard_factory_builder = timer_scabbard_factory_builder
                    .with_store_command_executor(Arc::new(
                        DieselStoreCommandExecutor::new_with_write_exclusivity(pool.clone()),
                    ));

                let timer_scabbard_factory: Box<dyn TimerHandlerFactory<Message = Vec<u8>>> =
                    Box::new(
                        timer_scabbard_factory_builder
                            .build()
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                    );

                let scabbard_timer_filter = ScabbardTimerFilter::new(Box::new(
                    scabbard::store::PooledSqliteScabbardStoreFactory::new_with_write_exclusivity(
                        pool.clone(),
                    ),
                ));

                timer_filter_collection
                    .push((Box::new(scabbard_timer_filter), timer_scabbard_factory));

                Ok(timer_filter_collection)
            }
            // This will have failed in create_store_factory above, but we return () to make
            // the compiler/linter happy under the following conditions
            #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
            store::ConnectionPool::Unsupported => Ok(timer_filter_collection),
        }
    }
}
