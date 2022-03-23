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
use crate::runtime::service::lifecycle_executor::executor::message::ExecutorMessage;
use crate::service::{FullyQualifiedServiceId, ServiceType};

use super::ExecutorAlarm;

pub struct ChannelExecutorAlarm {
    sender: Sender<ExecutorMessage>,
}

impl ChannelExecutorAlarm {
    pub fn new(sender: Sender<ExecutorMessage>) -> Self {
        ChannelExecutorAlarm { sender }
    }
}

impl ExecutorAlarm for ChannelExecutorAlarm {
    fn wake_up_all(&self) -> Result<(), InternalError> {
        self.sender
            .send(ExecutorMessage::WakeUpAll)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn wake_up(
        &self,
        service_type: ServiceType<'static>,
        service_id: Option<FullyQualifiedServiceId>,
    ) -> Result<(), InternalError> {
        self.sender
            .send(ExecutorMessage::WakeUp {
                service_type,
                service_id,
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
