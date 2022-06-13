// Copyright 2018-2022 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use splinter::service::{MessageHandlerFactory, Routable, ServiceType, TimerAlarmFactory};

use crate::store::PooledScabbardStoreFactory;

use super::message_handler::ScabbardMessageHandler;

const SCABBARD_SERVICE_TYPES: &[ServiceType<'static>] = &[ServiceType::new_static("scabbard:v3")];

#[derive(Clone)]
pub struct ScabbardMessageHandlerFactory {
    store_factory: Arc<dyn PooledScabbardStoreFactory>,
    timer_alarm_factory: Box<dyn TimerAlarmFactory>,
}

impl ScabbardMessageHandlerFactory {
    pub fn new(
        store_factory: Arc<dyn PooledScabbardStoreFactory>,
        timer_alarm_factory: Box<dyn TimerAlarmFactory>,
    ) -> Self {
        Self {
            store_factory,
            timer_alarm_factory,
        }
    }
}

impl MessageHandlerFactory for ScabbardMessageHandlerFactory {
    type MessageHandler = ScabbardMessageHandler;

    fn new_handler(&self) -> Self::MessageHandler {
        ScabbardMessageHandler::new(
            self.store_factory.new_store(),
            self.timer_alarm_factory.new_alarm(),
        )
    }

    fn clone_boxed(&self) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
        Box::new(self.clone())
    }
}

impl Routable for ScabbardMessageHandlerFactory {
    fn service_types(&self) -> &[ServiceType] {
        SCABBARD_SERVICE_TYPES
    }
}
