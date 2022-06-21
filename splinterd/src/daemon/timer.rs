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

use std::sync::mpsc::channel;
#[cfg(feature = "scabbardv3")]
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "scabbardv3")]
use scabbard::service::v3::{
    ScabbardTimerFilter, ScabbardTimerHandlerFactoryBuilder, Supervisor, SupervisorBuilder,
    SupervisorNotifierFactory,
};
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
use splinter::runtime::service::Timer;
use splinter::service::{TimerFilter, TimerHandlerFactory};
#[cfg(feature = "scabbardv3")]
use splinter::store::command::DieselStoreCommandExecutor;

use super::store::ConnectionPool;

#[cfg(feature = "service2")]
type TimerFilterCollection = Vec<(
    Box<dyn TimerFilter + Send>,
    Box<dyn TimerHandlerFactory<Message = Vec<u8>>>,
)>;

pub fn create_timer_and_supervisor(
    connection_pool: &ConnectionPool,
    node_id: &str,
    network_sender: NetworkMessageSender,
    routing_reader: Box<dyn RoutingTableReader>,
    service_timer_interval: &Duration,
) -> Result<(Timer, Supervisor), InternalError> {
    #[cfg_attr(not(feature = "scabbardv3"), allow(clippy::redundant_clone))]
    let mut timer_filter_collection: TimerFilterCollection = vec![];

    let (supervisor_sender, supervisor_recv) = channel();
    let supervisor_notifier_factory = SupervisorNotifierFactory::new(supervisor_sender.clone());

    #[cfg(feature = "scabbardv3")]
    {
        match connection_pool {
            #[cfg(feature = "database-postgres")]
            ConnectionPool::Postgres { pool } => {
                let supervisor_builder = SupervisorBuilder::new()
                    .with_pooled_scabbard_store_factory(Arc::new(
                        scabbard::store::PooledPgScabbardStoreFactory::new(pool.clone()),
                    ))
                    .with_scabbard_store_factory(Arc::new(PgScabbardStoreFactory))
                    .with_store_command_executor(Arc::new(DieselStoreCommandExecutor::new(
                        pool.clone(),
                    )))
                    .with_notifier_channel(supervisor_sender, supervisor_recv);

                let mut timer_scabbard_factory_builder = ScabbardTimerHandlerFactoryBuilder::new()
                    .with_message_sender_factory(Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender.clone(),
                        routing_reader.clone(),
                    )))
                    .with_supervisor_notifier_factory(supervisor_notifier_factory);

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

                let timer = Timer::new(
                    timer_filter_collection,
                    *service_timer_interval,
                    Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender,
                        routing_reader.clone(),
                    )),
                )?;

                let supervisor = supervisor_builder
                    .with_timer_alarm_factory(timer.alarm_factory())
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                Ok((timer, supervisor))
            }
            #[cfg(feature = "database-sqlite")]
            ConnectionPool::Sqlite { pool } => {
                let supervisor_builder = SupervisorBuilder::new()
                    .with_pooled_scabbard_store_factory(Arc::new(
                        scabbard::store::PooledSqliteScabbardStoreFactory::new_with_write_exclusivity(
                            pool.clone()),
                    ))
                    .with_scabbard_store_factory(Arc::new(SqliteScabbardStoreFactory))
                    .with_store_command_executor(Arc::new(DieselStoreCommandExecutor::new_with_write_exclusivity(
                        pool.clone(),
                    )))
                    .with_notifier_channel(supervisor_sender, supervisor_recv);

                let mut timer_scabbard_factory_builder = ScabbardTimerHandlerFactoryBuilder::new()
                    .with_message_sender_factory(Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender.clone(),
                        routing_reader.clone(),
                    )))
                    .with_supervisor_notifier_factory(supervisor_notifier_factory);
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

                let timer = Timer::new(
                    timer_filter_collection,
                    *service_timer_interval,
                    Box::new(NetworkMessageSenderFactory::new(
                        node_id,
                        network_sender,
                        routing_reader.clone(),
                    )),
                )?;

                let supervisor = supervisor_builder
                    .with_timer_alarm_factory(timer.alarm_factory())
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                Ok((timer, supervisor))
            }
            // This will have failed in create_store_factory above, but we return () to make
            // the compiler/linter happy under the following conditions
            #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
            store::ConnectionPool::Unsupported => Ok(timer_filter_collection),
        }
    }
}
