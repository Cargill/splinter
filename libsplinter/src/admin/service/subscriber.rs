// Copyright 2018-2021 Cargill Incorporated
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

//! Event Subscriber Map

use std::cell::RefCell;
use std::collections::HashMap;

use crate::admin::store::AdminServiceEvent;

use super::error::AdminSubscriberError;

pub trait AdminServiceEventSubscriber: Send {
    fn handle_event(
        &self,
        admin_service_event: &AdminServiceEvent,
    ) -> Result<(), AdminSubscriberError>;
}

pub struct SubscriberMap {
    subscribers_by_type: RefCell<HashMap<String, Vec<Box<dyn AdminServiceEventSubscriber>>>>,
}

impl SubscriberMap {
    pub fn new() -> Self {
        Self {
            subscribers_by_type: RefCell::new(HashMap::new()),
        }
    }

    pub fn broadcast_by_type(&self, event_type: &str, admin_service_event: &AdminServiceEvent) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        if let Some(subscribers) = subscribers_by_type.get_mut(event_type) {
            subscribers.retain(
                |subscriber| match subscriber.handle_event(admin_service_event) {
                    Ok(()) => true,
                    Err(AdminSubscriberError::Unsubscribe) => false,
                    Err(AdminSubscriberError::UnableToHandleEvent(msg)) => {
                        error!("Unable to send event: {}", msg);
                        true
                    }
                },
            );
        }
    }

    pub fn add_subscriber(
        &mut self,
        event_type: String,
        listener: Box<dyn AdminServiceEventSubscriber>,
    ) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        let subscribers = subscribers_by_type
            .entry(event_type)
            .or_insert_with(Vec::new);
        subscribers.push(listener);
    }

    pub fn clear(&mut self) {
        self.subscribers_by_type.borrow_mut().clear()
    }
}
