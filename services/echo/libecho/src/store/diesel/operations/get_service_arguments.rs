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
use splinter::{error::InternalError, service::FullyQualifiedServiceId, service::ServiceId};

use crate::service::EchoArguments;
use crate::store::diesel::{
    models::{EchoPeer, EchoService},
    schema::{echo_peers, echo_services},
};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait GetServiceArgumentsOperation {
    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError>;
}

impl<'a, C> GetServiceArgumentsOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    f32: diesel::deserialize::FromSql<diesel::sql_types::Float, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            let echo_service = echo_services::table
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .first::<EchoService>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .ok_or_else(|| {
                    InternalError::with_message(format!(
                        "Error retrieving service arguments, service ID {} does not exist",
                        service
                    ))
                })?;

            let peers: Vec<ServiceId> = match echo_peers::table
                .filter(echo_peers::service_id.eq(format!("{}", service)))
                .load::<EchoPeer>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
            {
                Some(peers) => peers
                    .into_iter()
                    .filter_map(|echo_peer| echo_peer.peer_service_id.map(ServiceId::new))
                    .collect::<Result<Vec<ServiceId>, _>>()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
                None => vec![],
            };

            let echo_args = match (
                echo_service.frequency,
                echo_service.jitter,
                echo_service.error_rate,
            ) {
                (Some(frequency), Some(jitter), Some(error_rate)) => EchoArguments::new(
                    peers,
                    std::time::Duration::from_millis(frequency as u64),
                    std::time::Duration::from_millis(jitter as u64),
                    error_rate,
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?,
                _ => {
                    return Err(InternalError::with_message(format!(
                        "Failed to get service arguments, service {} contains unset values",
                        service
                    )))
                }
            };

            Ok(echo_args)
        })
    }
}
