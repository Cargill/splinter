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

//! An alarm can be used to prematurely wake all or specific message handlers

use std::sync::mpsc::Sender;

use crate::error::InternalError;
use crate::runtime::service::timer::message::TimerMessage;
use crate::service::{FullyQualifiedServiceId, ServiceType, TimerAlarm, TimerAlarmFactory};

pub struct ChannelTimerAlarm {
    sender: Sender<TimerMessage>,
}

impl ChannelTimerAlarm {
    pub fn new(sender: Sender<TimerMessage>) -> Self {
        ChannelTimerAlarm { sender }
    }
}

impl TimerAlarm for ChannelTimerAlarm {
    /// Notify the `Timer` to check all `TimerFilters` for pending work
    fn wake_up_all(&self) -> Result<(), InternalError> {
        self.sender
            .send(TimerMessage::WakeUpAll)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    /// Notify the `Timer` to check a specific `TimerFilter` for pending work
    ///
    /// # Arguments
    ///
    /// * `service_type` - The service type of the the filter that will be checked
    /// * `service_id` - An optional service ID
    ///
    /// If a service ID is provided, only the `TimerHandler` for that ID will be run. The serivce
    /// ID must be returned from the `TimerFilter` to show there is pending work. If ther service
    /// ID is not returned, no handlers will be run.
    ///
    /// If the service ID is not provided, the handlers for all service IDs returned from the
    /// `TimerFilter` will be run.
    fn wake_up(
        &self,
        service_type: ServiceType<'static>,
        service_id: Option<FullyQualifiedServiceId>,
    ) -> Result<(), InternalError> {
        self.sender
            .send(TimerMessage::WakeUp {
                service_type,
                service_id,
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

pub struct ChannelTimerAlarmFactory {
    sender: Sender<TimerMessage>,
}

impl ChannelTimerAlarmFactory {
    pub fn new(sender: Sender<TimerMessage>) -> Self {
        ChannelTimerAlarmFactory { sender }
    }
}

/// Used to create new `TimerAlarm` instances.
impl TimerAlarmFactory for ChannelTimerAlarmFactory {
    /// Returns a new `TimerAlarm`
    fn new_alarm(&self) -> Result<Box<dyn TimerAlarm>, InternalError> {
        Ok(Box::new(ChannelTimerAlarm::new(self.sender.clone())))
    }

    fn clone_box(&self) -> Box<dyn TimerAlarmFactory> {
        Box::new(ChannelTimerAlarmFactory::new(self.sender.clone()))
    }
}
