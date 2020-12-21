// Copyright 2018-2020 Cargill Incorporated
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

//! Database backend support for the `AdminServiceEventStore`, powered by
//! [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselAdminServiceStore`].
//!
//! [`DieselAdminServiceEventStore`]: struct.DieselAdminServiceEventStore.html
//! [`AdminServiceEventStore`]: ../trait.AdminServiceEventStore.html

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use crate::admin::service::event::{
    store::{AdminServiceEventStore, AdminServiceEventStoreError, EventIter},
    AdminServiceEvent,
};
use crate::admin::service::messages;

/// A database-backed AdminServiceEventStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselAdminServiceEventStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselAdminServiceEventStore<C> {
    /// Creates a new `DieselAdminServiceEventStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn _new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselAdminServiceEventStore { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselAdminServiceEventStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl AdminServiceEventStore for DieselAdminServiceEventStore<diesel::sqlite::SqliteConnection> {
    fn add_event(
        &self,
        _event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceEventStoreError> {
        unimplemented!()
    }

    fn list_events_since(&self, _start: i64) -> Result<EventIter, AdminServiceEventStoreError> {
        unimplemented!()
    }

    fn list_events_by_management_type_since(
        &self,
        _management_type: String,
        _start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        unimplemented!()
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselAdminServiceEventStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl AdminServiceEventStore for DieselAdminServiceEventStore<diesel::pg::PgConnection> {
    fn add_event(
        &self,
        _event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceEventStoreError> {
        unimplemented!()
    }

    fn list_events_since(&self, _start: i64) -> Result<EventIter, AdminServiceEventStoreError> {
        unimplemented!()
    }

    fn list_events_by_management_type_since(
        &self,
        _management_type: String,
        _start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        unimplemented!()
    }
}
