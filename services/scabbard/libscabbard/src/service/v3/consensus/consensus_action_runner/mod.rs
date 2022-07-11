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

//! The `ConsensusActionRunner` is in charge of execution the actions that have been returned from
//! the consensus algorithms supported by Scabbard

mod commands;
mod context_updater;
mod notify_observer;

use std::convert::TryFrom;
use std::sync::Arc;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::service::MessageSenderFactory;
use splinter::store::command::StoreCommand;

use crate::store::Action;
use crate::store::ConsensusAction;
use crate::store::Identified;
use crate::store::ScabbardStoreFactory;

pub use self::commands::actions::ExecuteActionCommand;
pub use self::commands::context::UpdateContextCommand;
pub use self::context_updater::{ContextUpdater, ScabbardStoreContextUpdater};
pub use self::notify_observer::NotifyObserver;

/// Runs the actions provided from the consensus algorithm, in order of receipt.
pub struct ConsensusActionRunner<C> {
    message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    context_updater: Box<dyn ContextUpdater<C>>,
    notify_observer: Box<dyn NotifyObserver<C>>,
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C: 'static> ConsensusActionRunner<C> {
    /// Create a new ConsensusActionRunner
    ///
    /// # Arguments
    ///
    /// * `message_sender_factory` - Message sender factory used to get message senders for sending
    ///     consensus messages
    /// * `context_updater` - Updater to update conesus contexts and alarms
    /// * `notify_observer` - Observer for handling notifications that have been returned from
    ///     consensus
    /// * `store_factory` - Store factory used by commands that will update the scabbard store
    /// * `store_command_executor` - The executor for the commands that are returned from the other
    ///     components
    pub fn new(
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
        context_updater: Box<dyn ContextUpdater<C>>,
        notify_observer: Box<dyn NotifyObserver<C>>,
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    ) -> Self {
        ConsensusActionRunner {
            message_sender_factory,
            context_updater,
            notify_observer,
            store_factory,
        }
    }

    /// Runs the actions provided from the consensus algorithm, in order of receipt.
    ///
    /// Order is important as the algorithms requires each action to be completed before the next.
    /// All database operations that must be executed will be returned as a `StoreCommand` and
    /// executed at the same time as a command that marks the action as handled.
    ///
    /// # Arguments
    ///
    /// * `actions` - The list of action that must be processed, in order of execution
    /// * `service_id` - The service ID of the service the actions are for
    pub fn run_actions(
        &self,
        actions: Vec<Identified<ConsensusAction>>,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError> {
        let mut commands = Vec::new();
        for action in actions {
            match &action.record {
                ConsensusAction::TwoPhaseCommit(Action::Update(context, alarm)) => {
                    commands.extend(self.context_updater.update(
                        context.clone(),
                        service_id,
                        *alarm,
                    )?);

                    // add command to mark the action as executed
                    commands.push(Box::new(ExecuteActionCommand::new(
                        service_id.clone(),
                        action.id,
                        self.store_factory.clone(),
                    )));
                }
                ConsensusAction::TwoPhaseCommit(Action::SendMessage(to_service, msg)) => {
                    // close out notfication regardless of if this was succesful
                    let msg_bytes: Vec<u8> = Vec::<u8>::try_from(msg.clone())
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    let message_sender =
                        self.message_sender_factory.new_message_sender(service_id)?;
                    if let Err(err) = message_sender.send(to_service, msg_bytes) {
                        warn!(
                            "Unable to send consensus message to {}: {}",
                            to_service, err
                        );
                    }

                    // add command to mark the action as executed
                    commands.push(Box::new(ExecuteActionCommand::new(
                        service_id.clone(),
                        action.id,
                        self.store_factory.clone(),
                    )));
                }
                ConsensusAction::TwoPhaseCommit(Action::Notify(notification)) => {
                    commands.extend(self.notify_observer.notify(
                        notification.clone(),
                        service_id,
                        action.id,
                    )?);

                    // add command to mark the action as executed
                    commands.push(Box::new(ExecuteActionCommand::new(
                        service_id.clone(),
                        action.id,
                        self.store_factory.clone(),
                    )));
                }
            }
        }
        Ok(commands)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex,
    };
    use std::time::SystemTime;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
        Connection,
    };

    use splinter::service::{MessageSender, ServiceId};
    use splinter::store::command::StoreCommandExecutor;

    use crate::migrations::run_sqlite_migrations;
    use crate::service::v3::{SupervisorMessage, SupervisorNotifyObserver};
    use crate::store::pool::ConnectionPool;
    use crate::store::{
        AlarmType, ConsensusAction, ConsensusContext, ConsensusEvent, ConsensusType, Context,
        ContextBuilder, DieselScabbardStore, Event, Message, Notification, Participant,
        ScabbardService, ScabbardServiceBuilder, ScabbardStore, ServiceStatus,
        SqliteScabbardStoreFactory, State,
    };

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

    fn create_action_runner() -> (
        ConsensusActionRunner<SqliteConnection>,
        SqliteCommandExecutor,
        TestMessageSenderFactory,
        Box<dyn ScabbardStore>,
        Receiver<SupervisorMessage>,
    ) {
        let (_sender, recv) = channel();
        let pool = create_connection_pool_and_migrate();
        let store_command_executor = SqliteCommandExecutor {
            pool: pool.clone().into(),
        };
        let test_messsage_factory = TestMessageSenderFactory::default();
        let message_sender_factory = Box::new(test_messsage_factory.clone());

        let store_factory: Arc<(dyn ScabbardStoreFactory<diesel::sqlite::SqliteConnection>)> =
            Arc::new(SqliteScabbardStoreFactory);

        let context_updater = Box::new(ScabbardStoreContextUpdater::new(store_factory.clone()));
        let notify_observer = Box::new(SupervisorNotifyObserver::new(store_factory.clone()));

        let action_runner = ConsensusActionRunner::new(
            message_sender_factory,
            context_updater,
            notify_observer,
            store_factory,
        );

        let scabbard_store = Box::new(DieselScabbardStore::new(pool.clone()));

        (
            action_runner,
            store_command_executor,
            test_messsage_factory,
            scabbard_store,
            recv,
        )
    }

    fn create_context(service: &ScabbardService) -> Result<Context, InternalError> {
        let mut peers = service.peers().to_vec();

        peers.push(service.service_id().service_id().clone());

        let coordinator = get_coordinator(peers).ok_or_else(|| {
            InternalError::with_message(format!(
                "Unable to get coordinator service ID for service {}",
                service.service_id()
            ))
        })?;

        print!("Coordinatore {}", coordinator);

        if service.service_id().service_id() == &coordinator {
            ContextBuilder::default()
                .with_coordinator(&coordinator)
                .with_epoch(1)
                .with_participants(
                    service
                        .peers()
                        .iter()
                        .map(|participant| Participant {
                            process: participant.clone(),
                            vote: None,
                            decision_ack: false,
                        })
                        .collect(),
                )
                .with_state(State::WaitingForStart)
                .with_this_process(service.service_id().clone().service_id())
                .build()
                .map_err(|err| InternalError::from_source(Box::new(err)))
        } else {
            ContextBuilder::default()
                .with_coordinator(&coordinator)
                .with_epoch(1)
                .with_participants(
                    service
                        .peers()
                        .iter()
                        .map(|participant| Participant {
                            process: participant.clone(),
                            vote: None,
                            decision_ack: false,
                        })
                        .collect(),
                )
                .with_state(State::WaitingForVoteRequest)
                .with_this_process(service.service_id().clone().service_id())
                .build()
                .map_err(|err| InternalError::from_source(Box::new(err)))
        }
    }

    /// Gets the ID of the coordinator. The coordinator is the node with the lowest ID in the set of
    /// verifiers.
    fn get_coordinator(peers: Vec<ServiceId>) -> Option<ServiceId> {
        peers.into_iter().min_by(|x, y| x.as_str().cmp(y.as_str()))
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
                conn.transaction::<(), TestError, _>(|| {
                    for cmd in store_commands {
                        cmd.execute(conn).map_err(|err| TestError {
                            msg: err.to_string(),
                        })?;
                    }
                    Ok(())
                })
                .map_err(|err| InternalError::with_message(err.msg))
            })
        }
    }

    struct TestError {
        msg: String,
    }

    impl From<diesel::result::Error> for TestError {
        fn from(err: diesel::result::Error) -> Self {
            TestError {
                msg: err.to_string(),
            }
        }
    }

    /// Happy path test for the ConsensusActionRunner
    ///
    /// Verifies that one of each action type can be processed
    ///
    /// 1. Set up the intial state for the test
    ///    - Add a finalized service
    ///    - Add an initial context for the service
    /// 2. Add 3 pending Actions for to the store
    ///     - Update context that sets an alarms
    ///     - Send a message to the peer
    ///     - Notify of RequestForStart
    /// 3. Fetch pending actions from the scabbard store
    /// 4. Call run_actions on the ConsensusActionRunner, executing the Actions
    /// 5. Verify that no actions are returned after execution, meaning they have all ben udpated
    /// 6. Verify the service now has a consensus 2pc alarm set
    /// 7. Verify that a message was send to the peer
    /// 8. Verify a commit entry was added after RequestForStart
    #[test]
    fn test_consensus_action_runner() {
        let (action_runner, executor, message_sender_factory, scabbard_store, _recv) =
            create_action_runner();

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");
        let peer_service_id = ServiceId::new("bb00").unwrap();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        scabbard_store.add_service(service.clone()).unwrap();

        let context = create_context(&service).unwrap();

        scabbard_store
            .add_consensus_context(
                &service_fqsi,
                ConsensusContext::TwoPhaseCommit(context.clone()),
            )
            .expect("unable to add context to scabbard store");

        // add event
        scabbard_store
            .add_consensus_event(
                &service_fqsi,
                ConsensusEvent::TwoPhaseCommit(Event::Alarm()),
            )
            .expect("unable to add context to scabbard store");

        // add actions
        scabbard_store
            .add_consensus_action(
                ConsensusAction::TwoPhaseCommit(Action::Update(
                    ConsensusContext::TwoPhaseCommit(context),
                    Some(SystemTime::now()),
                )),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        scabbard_store
            .add_consensus_action(
                ConsensusAction::TwoPhaseCommit(Action::SendMessage(
                    peer_service_id.clone(),
                    Message::DecisionRequest(1),
                )),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        scabbard_store
            .add_consensus_action(
                ConsensusAction::TwoPhaseCommit(Action::Notify(Notification::RequestForStart())),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        let actions = scabbard_store
            .list_consensus_actions(&service_fqsi)
            .expect("unable to get actions");

        let commands = action_runner.run_actions(actions, &service_fqsi).unwrap();

        executor.execute(commands).unwrap();

        // verify that all actions were handled
        assert!(scabbard_store
            .list_consensus_actions(&service_fqsi)
            .expect("unable to get actions")
            .is_empty());

        let update_alarm = scabbard_store
            .get_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit)
            .expect("failed to get alarm");

        assert!(update_alarm.is_some());

        // Verify that a message was send to the peer
        let sent_messages = message_sender_factory.sent_messages.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        assert_eq!(sent_messages[0].0, peer_service_id);

        // verify a supervisor notification was added after RequestForStart
        assert!(
            scabbard_store
                .list_supervisor_notifications(&service_fqsi)
                .expect("unable to get commit entry")
                .len()
                == 1
        );
    }
}
