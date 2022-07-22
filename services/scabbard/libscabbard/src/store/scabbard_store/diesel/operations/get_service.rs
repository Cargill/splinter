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
use splinter::error::{InternalError, InvalidArgumentError};
use splinter::service::{FullyQualifiedServiceId, ServiceId};

use crate::store::scabbard_store::diesel::models::{ScabbardPeerModel, ScabbardServiceModel};
use crate::store::scabbard_store::diesel::{
    models::{
        ConsensusTypeModel, ConsensusTypeModelMapping, ServiceStatusTypeModel,
        ServiceStatusTypeModelMapping,
    },
    schema::{scabbard_peer, scabbard_service},
};
use crate::store::scabbard_store::{
    service::{ConsensusType, ScabbardServiceBuilder, ServiceStatus},
    ScabbardService, ScabbardStoreError,
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "get_service";

pub(in crate::store::scabbard_store::diesel) trait GetServiceOperation {
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError>;
}

impl<'a, C> GetServiceOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<ServiceStatusTypeModelMapping>,
    ServiceStatusTypeModel: diesel::deserialize::FromSql<ServiceStatusTypeModelMapping, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<ConsensusTypeModelMapping>,
    ConsensusTypeModel: diesel::deserialize::FromSql<ConsensusTypeModelMapping, C::Backend>,
{
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let service_model: ScabbardServiceModel = match scabbard_service::table
                .find((
                    &service_id.circuit_id().to_string(),
                    &service_id.service_id().to_string(),
                ))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })? {
                Some(service) => service,
                None => return Ok(None),
            };

            let mut service_peers: Vec<ScabbardPeerModel> = scabbard_peer::table
                .filter(
                    scabbard_peer::circuit_id
                        .eq(&service_id.circuit_id().to_string())
                        .and(scabbard_peer::service_id.eq(&service_id.service_id().to_string())),
                )
                .order(scabbard_peer::peer_service_id.asc())
                .load(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            service_peers.sort_by(|a, b| a.peer_service_id.cmp(&b.peer_service_id));

            let service_peers: Vec<ServiceId> = service_peers
                .into_iter()
                .map(|peer| ServiceId::new(peer.peer_service_id))
                .collect::<Result<Vec<_>, InvalidArgumentError>>()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            let service = ScabbardServiceBuilder::default()
                .with_service_id(service_id)
                .with_consensus(&ConsensusType::from(&service_model.consensus))
                .with_status(&ServiceStatus::from(&service_model.status))
                .with_peers(service_peers.as_slice())
                .build()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            Ok(Some(service))
        })
    }
}
