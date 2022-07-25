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

use crate::service::RequestStatus;
use crate::store::diesel::{models::EchoRequest, schema::echo_requests};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait UpdateRequestSentOperation {
    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError>;
}

impl<'a, C> UpdateRequestSentOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            if echo_requests::table
                .find((format!("{}", service), correlation_id))
                .get_result::<EchoRequest>(self.conn)
                .optional()
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .is_none()
            {
                return Err(InternalError::with_message(format!(
                    "Failed to update request, request with correlation ID {} does not exists",
                    &correlation_id
                )));
            }

            let update_sent_status = match sent {
                RequestStatus::Sent => 1,
                RequestStatus::NotSent => 0,
            };

            match sent_at {
                Some(_) => {
                    update(echo_requests::table)
                        .filter(
                            echo_requests::correlation_id
                                .eq(correlation_id)
                                .and(echo_requests::sender_service_id.eq(format!("{}", service))),
                        )
                        .set((
                            echo_requests::sent.eq(update_sent_status),
                            echo_requests::sent_at.eq(sent_at),
                        ))
                        .execute(self.conn)
                        .map(|_| ())
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                }
                None => {
                    update(echo_requests::table)
                        .filter(
                            echo_requests::correlation_id
                                .eq(correlation_id)
                                .and(echo_requests::sender_service_id.eq(format!("{}", service))),
                        )
                        .set(echo_requests::sent.eq(update_sent_status))
                        .execute(self.conn)
                        .map(|_| ())
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                }
            }

            Ok(())
        })
    }
}
