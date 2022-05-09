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
use splinter::store::command::StoreCommandExecutor;

use crate::store::action::IdentifiedScabbardConsensusAction;
use crate::store::two_phase::action::ConsensusAction;
use crate::store::ScabbardStoreFactory;

pub use self::commands::actions::ExecuteActionCommand;
pub use self::commands::context::UpdateContextCommand;
pub use self::commands::notifications::{
    AddCommitEntryCommand, AddEventCommand, UpdateCommitEntryCommand,
};
pub use self::context_updater::{ContextUpdater, ScabbardStoreContextUpdater};
pub use self::notify_observer::{CommandNotifyObserver, NotifyObserver};

/// Runs the actions provided from the consensus algorithm, in order of receipt.
pub struct ConsensusActionRunner<E: 'static>
where
    E: StoreCommandExecutor,
{
    message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    context_updater: Box<dyn ContextUpdater<E::Context>>,
    notify_observer: Box<dyn NotifyObserver<E::Context>>,
    store_factory: Arc<dyn ScabbardStoreFactory<E::Context>>,
    store_command_executor: E,
}

impl<E: 'static> ConsensusActionRunner<E>
where
    E: StoreCommandExecutor,
{
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
        context_updater: Box<dyn ContextUpdater<E::Context>>,
        notify_observer: Box<dyn NotifyObserver<E::Context>>,
        store_factory: Arc<dyn ScabbardStoreFactory<E::Context>>,
        store_command_executor: E,
    ) -> Self {
        ConsensusActionRunner {
            message_sender_factory,
            context_updater,
            notify_observer,
            store_command_executor,
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
    /// * `epoch` - The current epoch of the consensus algorithm
    pub fn run_actions(
        &self,
        actions: Vec<IdentifiedScabbardConsensusAction>,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<(), InternalError> {
        for action in actions {
            let mut commands = Vec::new();
            match &action {
                IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                    action_id,
                    ConsensusAction::Update(context, alarm),
                ) => {
                    commands.extend(self.context_updater.update(
                        context.clone(),
                        service_id,
                        *alarm,
                    )?);

                    // add command to mark the action as executed
                    commands.push(Box::new(ExecuteActionCommand::new(
                        service_id.clone(),
                        epoch,
                        *action_id,
                        self.store_factory.clone(),
                    )));
                }
                IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                    action_id,
                    ConsensusAction::SendMessage(to_service, msg),
                ) => {
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
                        epoch,
                        *action_id,
                        self.store_factory.clone(),
                    )));
                }
                IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                    action_id,
                    ConsensusAction::Notify(notification),
                ) => {
                    commands.extend(self.notify_observer.notify(
                        notification.clone(),
                        service_id,
                        epoch,
                    )?);

                    // add command to mark the action as executed
                    commands.push(Box::new(ExecuteActionCommand::new(
                        service_id.clone(),
                        epoch,
                        *action_id,
                        self.store_factory.clone(),
                    )));
                }
            }

            self.store_command_executor.execute(commands)?
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
        Connection,
    };

    use splinter::service::{MessageSender, ServiceId};
    use splinter::store::command::StoreCommand;

    use crate::migrations::run_sqlite_migrations;
    use crate::store::pool::ConnectionPool;
    use crate::store::{
        action::ScabbardConsensusAction,
        context::ScabbardContext,
        service::{ConsensusType, ScabbardService, ScabbardServiceBuilder, ServiceStatus},
        two_phase::action::ConsensusActionNotification,
        two_phase::context::{Context, ContextBuilder, Participant},
        two_phase::message::Scabbard2pcMessage,
        two_phase::state::Scabbard2pcState,
        DieselScabbardStore, ScabbardStore, SqliteScabbardStoreFactory,
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
        ConsensusActionRunner<SqliteCommandExecutor>,
        TestMessageSenderFactory,
        Box<dyn ScabbardStore>,
    ) {
        let pool = create_connection_pool_and_migrate();
        let store_command_executor = SqliteCommandExecutor {
            pool: pool.clone().into(),
        };
        let test_messsage_factory = TestMessageSenderFactory::default();
        let message_sender_factory = Box::new(test_messsage_factory.clone());

        let store_factory: Arc<(dyn ScabbardStoreFactory<diesel::sqlite::SqliteConnection>)> =
            Arc::new(SqliteScabbardStoreFactory);

        let context_updater = Box::new(ScabbardStoreContextUpdater::new(store_factory.clone()));
        let notify_observer = Box::new(CommandNotifyObserver::new(
            store_factory.clone(),
            Box::new(DieselScabbardStore::new(pool.clone())),
        ));

        let action_runner = ConsensusActionRunner::new(
            message_sender_factory,
            context_updater,
            notify_observer,
            store_factory,
            store_command_executor,
        );

        let scabbard_store = Box::new(DieselScabbardStore::new(pool.clone()));

        (action_runner, test_messsage_factory, scabbard_store)
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
                        })
                        .collect(),
                )
                .with_state(Scabbard2pcState::WaitingForStart)
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
                        })
                        .collect(),
                )
                .with_state(Scabbard2pcState::WaitingForVoteRequest)
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
    /// 6. Verify the context was updated with an alarm
    /// 7. Verify that a message was send to the peer
    /// 8. Verify a commit entry was added after RequestForStart
    #[test]
    fn test_consensus_action_runner() {
        let (action_runner, message_sender_factory, scabbard_store) = create_action_runner();

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

        let mut context = create_context(&service).unwrap();

        scabbard_store
            .add_consensus_context(
                &service_fqsi,
                ScabbardContext::Scabbard2pcContext(context.clone()),
            )
            .expect("unable to add context to scabbard store");

        context = context
            .into_builder()
            .with_alarm(SystemTime::now())
            .build()
            .unwrap();

        // add actions
        scabbard_store
            .add_consensus_action(
                ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Update(
                    ScabbardContext::Scabbard2pcContext(context),
                    Some(SystemTime::now()),
                )),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        scabbard_store
            .add_consensus_action(
                ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::SendMessage(
                    peer_service_id.clone(),
                    Scabbard2pcMessage::DecisionRequest(1),
                )),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        scabbard_store
            .add_consensus_action(
                ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Notify(
                    ConsensusActionNotification::RequestForStart(),
                )),
                &service_fqsi,
                1,
            )
            .expect("unable to add context to scabbard store");

        let actions = scabbard_store
            .list_consensus_actions(&service_fqsi, 1)
            .expect("unable to get actions");

        action_runner
            .run_actions(actions, &service_fqsi, 1)
            .unwrap();

        // verify that all actions were handled
        assert!(scabbard_store
            .list_consensus_actions(&service_fqsi, 1)
            .expect("unable to get actions")
            .is_empty());

        // verify the context was updated with an alarm
        let updated_context = match scabbard_store
            .get_current_consensus_context(&service_fqsi)
            .expect("unable to get commit entry")
            .expect("No commit entry returned")
        {
            ScabbardContext::Scabbard2pcContext(context) => context,
        };

        assert!(updated_context.alarm().is_some());

        // Verify that a message was send to the peer
        let sent_messages = message_sender_factory.sent_messages.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        assert_eq!(sent_messages[0].0, peer_service_id);

        // verify a commit entry was added after RequestForStart
        assert!(scabbard_store
            .get_last_commit_entry(&service_fqsi)
            .expect("unable to get commit entry")
            .is_some());
    }
}
