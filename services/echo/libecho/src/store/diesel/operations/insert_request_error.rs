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

use std::convert::TryFrom;

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{dsl::insert_into, prelude::*, result::Error::NotFound};
use splinter::{error::InternalError, service::FullyQualifiedServiceId};

use crate::store::diesel::{
    models::{EchoRequestError, EchoService},
    schema::{echo_request_errors, echo_services},
};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait InsertRequestErrorOperation {
    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError>;
}

#[cfg(feature = "sqlite")]
impl<'a> InsertRequestErrorOperation for EchoStoreOperations<'a, SqliteConnection> {
    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
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
                    "Failed to add request, service ID {} does not exist",
                    service
                )));
            }

            let previous_correlation_id: i64 = echo_request_errors::table
                .order(echo_request_errors::correlation_id.desc())
                .select(echo_request_errors::correlation_id)
                .first::<i64>(self.conn)
                .optional()?
                .unwrap_or(0);

            let new_request_error = EchoRequestError {
                service_id: format!("{}", service),
                correlation_id: previous_correlation_id + 1,
                error_message: error_message.into(),
                error_at,
            };
            insert_into(echo_request_errors::table)
                .values(vec![new_request_error])
                .execute(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            let correlation_id = u64::try_from(
                echo_request_errors::table
                    .order(echo_request_errors::correlation_id.desc())
                    .select(echo_request_errors::correlation_id)
                    .first::<i64>(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            )
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

            Ok(correlation_id)
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> InsertRequestErrorOperation for EchoStoreOperations<'a, PgConnection> {
    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError> {
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
                    "Failed to add request, service ID {} does not exist",
                    service
                )));
            }

            let previous_correlation_id: i64 = echo_request_errors::table
                .order(echo_request_errors::correlation_id.desc())
                .select(echo_request_errors::correlation_id)
                .first::<i64>(self.conn)
                .optional()?
                .unwrap_or(0);

            let new_request_error = EchoRequestError {
                service_id: format!("{}", service),
                correlation_id: previous_correlation_id + 1,
                error_message: error_message.into(),
                error_at,
            };
            let correlation_id = u64::try_from(
                insert_into(echo_request_errors::table)
                    .values(vec![new_request_error])
                    .returning(echo_request_errors::correlation_id)
                    .get_result::<i64>(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            )
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

            Ok(correlation_id)
        })
    }
}
