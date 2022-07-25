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

use diesel::prelude::*;
use splinter::{error::InternalError, service::FullyQualifiedServiceId, service::ServiceId};

use crate::store::diesel::schema::echo_requests;

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait GetLastSentOperation {
    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError>;
}

impl<'a, C> GetLastSentOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    f32: diesel::deserialize::FromSql<diesel::sql_types::Float, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            Ok(echo_requests::table
                .filter(
                    echo_requests::sender_service_id
                        .eq(format!("{}", sender_service_id))
                        .and(
                            echo_requests::receiver_service_id
                                .eq(format!("{}", receiver_service_id)),
                        )
                        .and(echo_requests::sent.eq(1)),
                )
                .select(echo_requests::sent_at.nullable())
                .order(echo_requests::sent_at.desc())
                .load::<Option<i64>>(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .pop()
                .flatten())
        })
    }
}
