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

//! Provides database operations for the `DieselAdminServiceStore`.

#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod add_circuit;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod add_event;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod add_proposal;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod count_circuits;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod count_proposals;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod get_circuit;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod get_node;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod get_proposal;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod get_service;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_circuits;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_events;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_events_by_management_type_since;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_events_since;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_nodes;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_proposals;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod list_services;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod remove_circuit;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod remove_proposal;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod update_circuit;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod update_proposal;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(super) mod upgrade;

pub struct AdminServiceStoreOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C: diesel::Connection> AdminServiceStoreOperations<'a, C> {
    pub fn new(conn: &'a C) -> Self {
        AdminServiceStoreOperations { conn }
    }
}
