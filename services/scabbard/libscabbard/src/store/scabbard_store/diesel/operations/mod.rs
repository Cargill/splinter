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

pub(super) mod add_commit_entry;
pub(super) mod add_consensus_action;
pub(super) mod add_consensus_context;
pub(super) mod add_consensus_event;
pub(super) mod add_service;
pub(super) mod add_supervisor_notification;
pub(super) mod get_alarm;
pub(super) mod get_current_consensus_context;
pub(super) mod get_last_commit_entry;
pub(super) mod get_service;
pub(super) mod list_consensus_actions;
pub(super) mod list_consensus_events;
pub(super) mod list_ready_services;
pub(super) mod list_supervisor_notifications;
pub(super) mod remove_service;
pub(super) mod set_alarm;
pub(super) mod unset_alarm;
pub(super) mod update_commit_entry;
pub(super) mod update_consensus_action;
pub(super) mod update_consensus_context;
pub(super) mod update_consensus_event;
pub(super) mod update_service;
pub(super) mod update_supervisor_notification;

pub struct ScabbardStoreOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C: diesel::Connection> ScabbardStoreOperations<'a, C> {
    pub fn new(conn: &'a C) -> Self {
        ScabbardStoreOperations { conn }
    }
}
