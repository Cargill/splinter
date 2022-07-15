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

use diesel::{prelude::*, result::Error::NotFound, update};
use splinter::{error::InternalError, service::FullyQualifiedServiceId};

use crate::service::EchoServiceStatus;
use crate::store::diesel::{models::EchoService, schema::echo_services};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait UpdateServiceStatusOperation {
    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError>;
}

impl<'a, C> UpdateServiceStatusOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    f32: diesel::deserialize::FromSql<diesel::sql_types::Float, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            if echo_services::table
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .first::<EchoService>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .is_none()
            {
                return Err(InternalError::with_message(format!(
                    "Failed to update echo service, service ID {} does not exist",
                    service
                )));
            }

            let update_status = match status {
                EchoServiceStatus::Prepared => 1,
                EchoServiceStatus::Finalized => 2,
                EchoServiceStatus::Retired => 3,
            };

            update(echo_services::table)
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .set(echo_services::status.eq(update_status))
                .execute(self.conn)
                .map(|_| ())
                .map_err(|err| InternalError::from_source(Box::new(err)))
        })
    }
}
