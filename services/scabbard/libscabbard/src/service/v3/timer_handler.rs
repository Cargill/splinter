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

use splinter::{
    error::InternalError,
    service::{FullyQualifiedServiceId, MessageSender, TimerHandler},
    store::command::StoreCommandExecutor,
};

use crate::protocol::v3::message::ScabbardMessage;
use crate::service::v3::supervisor::SupervisorNotifier;
use crate::store::{AlarmType, ConsensusEvent, Event, ScabbardStore};

use super::ConsensusRunner;

pub struct ScabbardTimerHandler<E>
where
    E: StoreCommandExecutor + 'static,
{
    consensus_runner: ConsensusRunner<E>,
    store: Box<dyn ScabbardStore>,
    supervisor_notifier: SupervisorNotifier,
}

impl<E: StoreCommandExecutor + 'static> ScabbardTimerHandler<E> {
    pub fn new(
        consensus_runner: ConsensusRunner<E>,
        store: Box<dyn ScabbardStore>,
        supervisor_notifier: SupervisorNotifier,
    ) -> Self {
        Self {
            consensus_runner,
            store,
            supervisor_notifier,
        }
    }
}

impl<E: StoreCommandExecutor + 'static> TimerHandler for ScabbardTimerHandler<E> {
    type Message = ScabbardMessage;

    fn handle_timer(
        &mut self,
        _sender: &dyn MessageSender<Self::Message>,
        service: FullyQualifiedServiceId,
    ) -> Result<(), InternalError> {
        // Check store if we have an alarm that have expired
        if let Some(alarm) = self
            .store
            .get_alarm(&service, &AlarmType::TwoPhaseCommit)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
        {
            // If the alarm has passed, elapsed will return OK and a duration for how long ago
            // it was. If it is still in the future an error is returned with how long until
            // the alarm has passed. Therefore if ok, the alarm has passed and an alarm event
            // should be added to state.
            if alarm.elapsed().is_ok() {
                let alarm_event = ConsensusEvent::TwoPhaseCommit(Event::Alarm());
                self.store
                    .add_consensus_event(&service, alarm_event)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
            }
        }

        // always run the consensus runner, there is some pending work
        self.consensus_runner.run(&service)?;
        self.supervisor_notifier.notify(&service)
    }
}
