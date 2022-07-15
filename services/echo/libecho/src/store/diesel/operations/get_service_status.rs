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

use diesel::{prelude::*, result::Error::NotFound};
use splinter::{error::InternalError, service::FullyQualifiedServiceId};

use crate::service::EchoServiceStatus;
use crate::store::diesel::schema::echo_services;

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait GetServiceStatusOperation {
    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError>;
}

impl<'a, C> GetServiceStatusOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
{
    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            echo_services::table
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .select(echo_services::status)
                .first::<i16>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .ok_or_else(|| {
                    InternalError::with_message(format!(
                        "Error retrieving service status, service ID {} does not exist",
                        service
                    ))
                })
                .map(|s| match s {
                    1 => EchoServiceStatus::Prepared,
                    2 => EchoServiceStatus::Finalized,
                    _ => EchoServiceStatus::Retired,
                })
        })
    }
}
