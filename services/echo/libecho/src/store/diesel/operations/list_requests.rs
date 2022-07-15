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

use diesel::prelude::*;
use splinter::{error::InternalError, service::FullyQualifiedServiceId, service::ServiceId};

use crate::service::EchoRequest;
use crate::store::diesel::{models::EchoRequest as EchoRequestModel, schema::echo_requests};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait ListRequestsOperation {
    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError>;
}

impl<'a, C> ListRequestsOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
{
    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError> {
        self.conn
            .transaction::<_, _, _>(|| match receiver_service_id {
                Some(receiver_service_id) => echo_requests::table
                    .filter(
                        echo_requests::sender_service_id
                            .eq(format!("{}", service))
                            .and(
                                echo_requests::receiver_service_id
                                    .eq(format!("{}", receiver_service_id)),
                            ),
                    )
                    .select(echo_requests::all_columns)
                    .load::<EchoRequestModel>(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .into_iter()
                    .map(EchoRequest::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| InternalError::from_source(Box::new(err))),
                None => echo_requests::table
                    .filter(echo_requests::sender_service_id.eq(format!("{}", service)))
                    .select(echo_requests::all_columns)
                    .load::<EchoRequestModel>(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .into_iter()
                    .map(EchoRequest::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| InternalError::from_source(Box::new(err))),
            })
    }
}
