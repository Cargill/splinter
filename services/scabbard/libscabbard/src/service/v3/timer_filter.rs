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

use crate::store::PooledScabbardStoreFactory;

const STATIC_TYPES: &[ServiceType] = &[ServiceType::new_static("scabbard:v3")];

pub struct ScabbardTimerFilter {
    store_factory: Box<dyn PooledScabbardStoreFactory>,
}

impl ScabbardTimerFilter {
    pub fn new(store_factory: Box<dyn PooledScabbardStoreFactory>) -> Self {
        Self { store_factory }
    }
}

impl TimerFilter for ScabbardTimerFilter {
    fn filter(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        self.store_factory
            .new_store()
            .list_ready_services()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

impl Routable for ScabbardTimerFilter {
    fn service_types(&self) -> &[ServiceType] {
        STATIC_TYPES
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, RwLock};

    use std::time::{Duration, SystemTime};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use splinter::service::ServiceId;

    use crate::migrations::run_sqlite_migrations;
    use crate::store::service::ServiceStatus;
    use crate::store::service::{ConsensusType, ScabbardServiceBuilder};
    use crate::store::DieselScabbardStore;
    use crate::store::PooledSqliteScabbardStoreFactory;
    use crate::store::ScabbardStore;
    use crate::store::{
        action::ScabbardConsensusAction,
        context::ConsensusContext,
        two_phase::{
            action::{Action, ConsensusActionNotification},
            context::{ContextBuilder, Participant},
            state::Scabbard2pcState,
        },
    };

    /// Test that the `ScabbardTimerFilter`'s `filter` function works
    ///
    /// 1. Add two services in the finalized state to the database
    /// 2. Add a context with a past due alarm and an unexecuted action for the first service
    /// 3. Create a new `ScabbardTimerFilter` and call the `filter` method, check that only the
    ///    first service is returned
    /// 4. Add a context with a past due alarm for the second service
    /// 5. Call `filter` and check that both service IDs are now returned
    /// 6. Add an unexecuted action for the second service
    /// 7. Call `filter` and check that both service IDs are now returned
    /// 8. Update the alarms for both services to a week from now
    /// 9. Update the `executed_at` time for the second service's action
    /// 10. Call `filter` and check that the first service IDs is returned because it still has
    ///     outstanding actions
    /// 11. Update the `executed_at` time for the first service's action
    /// 10. Call `filter` and check that no service IDs are returned because there are no services
    ///     with past due timers or outstanding actions
    #[test]
    fn test_scabbard_timer_filter() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool.clone());

        let fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let fqsi2 = FullyQualifiedServiceId::new_from_string("abcde-fghij::bb00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::bb00'");

        let peer_service1 =
            ServiceId::new(String::from("bb00")).expect("failed to make service ID aa00");
        let peer_service2 =
            ServiceId::new(String::from("aa00")).expect("failed to make service ID bb00");

        let service = ScabbardServiceBuilder::default()
            .with_service_id(&fqsi)
            .with_peers(&[peer_service1.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        let service2 = ScabbardServiceBuilder::default()
            .with_service_id(&fqsi2)
            .with_peers(&[peer_service2.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("failed to add service");
        store.add_service(service2).expect("failed to add service2");

        let coordinator_context = ContextBuilder::default()
            .with_alarm(SystemTime::now())
            .with_coordinator(fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service1.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        // add a coordinator context for the first service
        store
            .add_consensus_context(&fqsi, context)
            .expect("failed to add context to store");

        let notification = ConsensusActionNotification::RequestForStart();
        let action =
            ScabbardConsensusAction::Scabbard2pcConsensusAction(Action::Notify(notification));

        // add an unexecuted action for the first service
        let action_id = store
            .add_consensus_action(action, &fqsi, 1)
            .expect("failed to add action");

        let scabbard_timer_filter = ScabbardTimerFilter::new(Box::new(
            PooledSqliteScabbardStoreFactory::new_with_write_exclusivity(Arc::new(RwLock::new(
                pool,
            ))),
        ));

        let ids = scabbard_timer_filter.filter().expect("failed to filter");

        // check that the service with a past due alarm is listed
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&fqsi));

        let participant_context = ContextBuilder::default()
            .with_alarm(SystemTime::now())
            .with_coordinator(fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service1.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForVoteRequest)
            .with_this_process(fqsi2.clone().service_id())
            .build()
            .expect("failed to build context");
        let context2 = ConsensusContext::TwoPhaseCommit(participant_context);

        // add a context for the second service
        store
            .add_consensus_context(&fqsi2, context2)
            .expect("failed to add context to store");

        let ids = scabbard_timer_filter.filter().expect("failed to filter");

        // check that both services are listed because both have past due alarms
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&fqsi));
        assert!(ids.contains(&fqsi2));

        let notification2 =
            ConsensusActionNotification::MessageDropped("test dropped message".to_string());
        let action2 =
            ScabbardConsensusAction::Scabbard2pcConsensusAction(Action::Notify(notification2));

        // add an unexecuted action for the second service
        let action_id2 = store
            .add_consensus_action(action2, &fqsi2, 1)
            .expect("failed to add action2");

        let ids = scabbard_timer_filter.filter().expect("failed to filter");

        // check that both services are still listed
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&fqsi));
        assert!(ids.contains(&fqsi2));

        let updated_alarm = SystemTime::now()
            .checked_add(Duration::from_secs(604800))
            .expect("failed to get alarm time");

        let update_context2 = ContextBuilder::default()
            .with_alarm(updated_alarm)
            .with_coordinator(fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service1.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForVoteRequest)
            .with_this_process(fqsi2.clone().service_id())
            .build()
            .expect("failed to build context");

        // reset the alarms for both services to far in the future
        store
            .update_consensus_context(&fqsi2, ConsensusContext::TwoPhaseCommit(update_context2))
            .expect("failed to update context");

        let updated_alarm = SystemTime::now()
            .checked_add(Duration::from_secs(604800))
            .expect("failed to get alarm time");

        let update_context1 = ContextBuilder::default()
            .with_alarm(updated_alarm)
            .with_coordinator(fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service1.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        store
            .update_consensus_context(&fqsi, ConsensusContext::TwoPhaseCommit(update_context1))
            .expect("failed to update context");

        // update the second service's action's executed_at time so that it appears to have
        // been executed
        store
            .update_consensus_action(&fqsi2, 1, action_id2, SystemTime::now())
            .expect("failed to update action");

        let ids = scabbard_timer_filter.filter().expect("failed to filter");

        // check that only the first service is listed because it has outstanding actions
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&fqsi));

        // update the first service's action's executed_at time so that it appears to have
        // been executed
        store
            .update_consensus_action(&fqsi, 1, action_id, SystemTime::now())
            .expect("failed to update action");

        let ids = scabbard_timer_filter.filter().expect("failed to filter");

        // check that no services are listed
        assert_eq!(ids.len(), 0);
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
