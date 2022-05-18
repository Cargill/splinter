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

mod action_source;
mod builder;
mod consensus_store_command_factory;
mod context_source;
mod event_source;
mod store_sources;

use std::collections::HashMap;

use augrim::Algorithm;
use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommandExecutor;

use crate::store::ConsensusAction;
use crate::store::ConsensusContext;
use crate::store::ConsensusEvent;

use super::ConsensusActionRunner;

pub use action_source::UnprocessedActionSource;
pub use builder::ConsensusRunnerBuilder;
use consensus_store_command_factory::ConsensusStoreCommandFactory;
pub use context_source::ContextSource;
pub use event_source::UnprocessedEventSource;
pub use store_sources::{
    StoreContextSource, StoreUnprocessedActionSource, StoreUnprocessedEventSource,
};

pub struct ConsensusRunner<E>
where
    E: StoreCommandExecutor + 'static,
{
    unprocessed_action_source: Box<dyn UnprocessedActionSource>,
    action_runner: ConsensusActionRunner<<E as StoreCommandExecutor>::Context>,
    unprocessed_event_source: Box<dyn UnprocessedEventSource>,
    context_source: Box<dyn ContextSource>,
    algorithms: HashMap<
        String,
        Box<
            dyn Algorithm<
                Event = ConsensusEvent,
                Action = ConsensusAction,
                Context = ConsensusContext,
            >,
        >,
    >,
    consensus_store_command_factory:
        ConsensusStoreCommandFactory<<E as StoreCommandExecutor>::Context>,
    store_command_executor: E,
}

impl<E> ConsensusRunner<E>
where
    E: StoreCommandExecutor,
    <E as StoreCommandExecutor>::Context: 'static,
{
    pub fn run(&self, service_id: &FullyQualifiedServiceId) -> Result<(), InternalError> {
        loop {
            let context = self
                .context_source
                .get_context(service_id)?
                .ok_or_else(|| {
                    InternalError::with_message(format!(
                        "No scabbard context for service {}",
                        service_id
                    ))
                })?;

            let unprocessed_actions = self
                .unprocessed_action_source
                .get_unprocessed_actions(service_id)?;

            let mut commands = vec![];
            if !unprocessed_actions.is_empty() {
                commands.extend(
                    self.action_runner
                        .run_actions(unprocessed_actions, service_id)?,
                );
            }

            let unprocessed_event = self.unprocessed_event_source.get_next_event(service_id)?;

            let event = match unprocessed_event {
                Some(event) => event,
                None => {
                    if !commands.is_empty() {
                        self.store_command_executor.execute(commands)?;
                        continue;
                    } else {
                        // No actions and no events
                        break Ok(());
                    }
                }
            };

            let (event_id, event) = event.deconstruct();

            let algorithm = self.algorithms.get(event.algorithm_name()).ok_or_else(|| {
                InternalError::with_message(format!("{} is not configured", event.algorithm_name()))
            })?;
            let actions = algorithm
                .event(event, context)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            commands.push(
                self.consensus_store_command_factory
                    .new_save_actions_command(service_id, actions),
            );
            commands.push(
                self.consensus_store_command_factory
                    .new_mark_event_complete_command(service_id, event_id),
            );
            self.store_command_executor.execute(commands)?;
        }
    }
}

#[cfg(test)]
mod tests {
    pub use super::*;

    use std::sync::{Arc, Mutex};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
        Connection,
    };

    use augrim::two_phase_commit::TwoPhaseCommitAlgorithm;
    use splinter::service::{MessageSender, MessageSenderFactory, ServiceId};
    use splinter::store::command::StoreCommand;

    use crate::migrations::run_sqlite_migrations;
    use crate::service::v3::CommandNotifyObserver;
    use crate::store::pool::ConnectionPool;
    use crate::store::{
        AlarmType, ConsensusContext, ConsensusType, ContextBuilder, DieselScabbardStore, Event,
        Participant, ScabbardServiceBuilder, ScabbardStore, ServiceStatus,
        SqliteScabbardStoreFactory, State,
    };

    use self::store_sources::{
        StoreContextSource, StoreUnprocessedActionSource, StoreUnprocessedEventSource,
    };

    /// Test that the ConsensusRunner properly handles an event and the resulting actions
    ///
    /// 1. Creates a ScabbardService with the service ID AAAAA-bbbbb::test
    /// 2. Create a context for the service with an Epoch of 1
    /// 3. Add the service and the context to state
    /// 4. Add an Event::Start event to the database, this should result in two actions
    ///      a. SendMessage
    ///      b. Context update that includes an Alarm
    /// 5. Create the ConsensusRunner and call run()
    /// 6. Verify that a message was sent to our peer
    /// 7. Verify that an alarm was set
    #[test]
    fn test_run_start_event() -> Result<(), Box<dyn std::error::Error>> {
        let pool = create_connection_pool_and_migrate();
        let scabbard_store = Box::new(DieselScabbardStore::new(pool.clone()));

        let service_id = FullyQualifiedServiceId::new_from_string("AAAAA-bbbbb::test")?;
        let peer_service_id = ServiceId::new("bb00").unwrap();
        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_id)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        let current_context = ConsensusContext::TwoPhaseCommit(
            ContextBuilder::new()
                .with_coordinator(service_id.service_id())
                .with_epoch(1)
                .with_state(State::WaitingForVote)
                .with_this_process(service_id.service_id())
                .with_participants(vec![Participant {
                    process: peer_service_id.clone(),
                    vote: None,
                }])
                .build()?,
        );

        scabbard_store.add_service(service.clone()).unwrap();

        scabbard_store
            .add_consensus_context(&service_id, current_context.clone())
            .expect("unable to add context to scabbard store");

        scabbard_store
            .add_consensus_event(
                &service_id,
                ConsensusEvent::TwoPhaseCommit(Event::Start(b"test".to_vec())),
            )
            .expect("unable to event to the scabbard store");

        let store_command_executor = SqliteCommandExecutor {
            pool: pool.clone().into(),
        };
        let test_messsage_factory = TestMessageSenderFactory::default();

        let message_sender_factory = Box::new(test_messsage_factory.clone());
        let store_factory = Arc::new(SqliteScabbardStoreFactory);

        let notify_observer = Box::new(CommandNotifyObserver::new(
            store_factory.clone(),
            Box::new(DieselScabbardStore::new(pool.clone())),
        ));

        let runner = ConsensusRunnerBuilder::new()
            .with_scabbard_store_factory(store_factory)
            .with_unprocessed_action_source(Box::new(StoreUnprocessedActionSource::new(Box::new(
                DieselScabbardStore::new(pool.clone()),
            ))))
            .with_unprocessed_event_source(Box::new(StoreUnprocessedEventSource::new(Box::new(
                DieselScabbardStore::new(pool.clone()),
            ))))
            .with_store_command_executor(store_command_executor)
            .with_context_source(Box::new(StoreContextSource::new(Box::new(
                DieselScabbardStore::new(pool.clone()),
            ))))
            .with_message_sender_factory(message_sender_factory)
            .with_notify_observer(notify_observer)
            .with_algorithm(
                "two-phase-commit",
                Box::new(
                    TwoPhaseCommitAlgorithm::new(augrim::SystemTimeFactory::new()).into_algorithm(),
                ),
            )
            .build()?;

        // runner should handle 1 event(Event::Start), which should result in to actions,
        // send message and update context
        runner.run(&service_id)?;

        let sent_messages = test_messsage_factory.sent_messages.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        assert_eq!(sent_messages[0].0, peer_service_id);

        let update_alarm = scabbard_store
            .get_alarm(&service_id, &AlarmType::TwoPhaseCommit)
            .expect("failed to get alarm");

        assert!(update_alarm.is_some());

        Ok(())
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

    struct SqliteCommandExecutor {
        pool: ConnectionPool<SqliteConnection>,
    }

    impl StoreCommandExecutor for SqliteCommandExecutor {
        type Context = SqliteConnection;

        fn execute<C: StoreCommand<Context = SqliteConnection>>(
            &self,
            store_commands: Vec<C>,
        ) -> Result<(), InternalError> {
            self.pool.execute_write(|conn| {
                let res: Result<(), InternalError> =
                    conn.transaction::<(), InternalError, _>(|| {
                        for cmd in store_commands {
                            let res: Result<(), InternalError> = cmd.execute(conn);

                            res?
                        }
                        Ok(())
                    });

                res
            })
        }
    }

    struct TestMessageSender {
        pub sent_messages: Arc<Mutex<Vec<(ServiceId, Vec<u8>)>>>,
    }

    impl MessageSender<Vec<u8>> for TestMessageSender {
        fn send(&self, to_service: &ServiceId, message: Vec<u8>) -> Result<(), InternalError> {
            self.sent_messages
                .lock()
                .unwrap()
                .push((to_service.clone(), message));
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct TestMessageSenderFactory {
        pub sent_messages: Arc<Mutex<Vec<(ServiceId, Vec<u8>)>>>,
    }

    impl MessageSenderFactory<Vec<u8>> for TestMessageSenderFactory {
        /// Returns a new `MessageSender`
        fn new_message_sender(
            &self,
            _scope: &FullyQualifiedServiceId,
        ) -> Result<Box<dyn MessageSender<Vec<u8>>>, InternalError> {
            Ok(Box::new(TestMessageSender {
                sent_messages: self.sent_messages.clone(),
            }))
        }

        fn clone_boxed(&self) -> Box<dyn MessageSenderFactory<Vec<u8>>> {
            Box::new(self.clone())
        }
    }
}
