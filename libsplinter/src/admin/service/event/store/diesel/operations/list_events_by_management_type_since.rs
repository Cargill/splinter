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

//! Provides the "list events by management type" operation for the `DieselAdminServiceEventStore`.

use diesel::{prelude::*, types::HasSqlType};

use super::{
    list_events::AdminServiceEventStoreListEventsOperation, AdminServiceEventStoreOperations,
};

use crate::admin::service::event::store::{
    diesel::schema::admin_event_proposed_circuit, AdminServiceEventStoreError, EventIter,
};

pub(in crate::admin::service::event::store::diesel) trait AdminServiceEventStoreListEventsByManagementTypeSinceOperation
{
    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError>;
}

impl<'a, C> AdminServiceEventStoreListEventsByManagementTypeSinceOperation
    for AdminServiceEventStoreOperations<'a, C>
where
    C: diesel::Connection,
    C::Backend: HasSqlType<diesel::sql_types::BigInt>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    Vec<u8>: diesel::deserialize::FromSql<diesel::sql_types::Binary, C::Backend>,
{
    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        self.conn.transaction::<EventIter, _, _>(|| {
            let event_ids: Vec<i64> = admin_event_proposed_circuit::table
                .filter(admin_event_proposed_circuit::event_id.gt(start))
                .filter(admin_event_proposed_circuit::circuit_management_type.eq(management_type))
                .select(admin_event_proposed_circuit::event_id)
                .load(self.conn)?;
            AdminServiceEventStoreOperations::new(self.conn).list_events(event_ids)
        })
    }
}
