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

//! Provides the "list events since" operation for the `DieselAdminServiceStore`.

use diesel::{prelude::*, types::HasSqlType};

use super::{list_events::AdminServiceStoreListEventsOperation, AdminServiceStoreOperations};

use crate::admin::store::{diesel::schema::admin_service_event, AdminServiceStoreError, EventIter};

pub(in crate::admin::store::diesel) trait AdminServiceStoreListEventsSinceOperation {
    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreListEventsSinceOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    C::Backend: HasSqlType<diesel::sql_types::BigInt>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    Vec<u8>: diesel::deserialize::FromSql<diesel::sql_types::Binary, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
{
    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceStoreError> {
        self.conn.transaction::<EventIter, _, _>(|| {
            let event_ids: Vec<i64> = admin_service_event::table
                .filter(admin_service_event::id.gt(start))
                .select(admin_service_event::id)
                .load(self.conn)?;
            AdminServiceStoreOperations::new(self.conn).list_events(event_ids)
        })
    }
}
