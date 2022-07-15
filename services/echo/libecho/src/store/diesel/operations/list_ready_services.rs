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
use splinter::{error::InternalError, service::FullyQualifiedServiceId};

use crate::store::diesel::schema::{echo_peers, echo_services};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait ListReadyServicesOperation {
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError>;
}

impl<'a, C> ListReadyServicesOperation for EchoStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            // get all services that have peers
            let services_with_peers: Vec<String> = echo_peers::table
                .filter(echo_peers::peer_service_id.is_not_null())
                .select(echo_peers::service_id)
                .load::<String>(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .into_iter()
                .collect();

            // of the services with peers get the services that are in the `Finalized` state
            echo_services::table
                .filter(
                    echo_services::service_id
                        .eq_any(services_with_peers)
                        .and(echo_services::status.eq(2)),
                )
                .select(echo_services::service_id)
                .load::<String>(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .into_iter()
                .map(FullyQualifiedServiceId::new_from_string)
                .collect::<Result<Vec<FullyQualifiedServiceId>, _>>()
                .map_err(|err| InternalError::from_source(Box::new(err)))
        })
    }
}
