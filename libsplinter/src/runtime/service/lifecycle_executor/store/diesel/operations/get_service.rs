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

//! Provides the "get service" operation for the `DieselLifecycleStore`.

use diesel::prelude::*;

use super::LifecycleStoreOperations;
use crate::runtime::service::{
    lifecycle_executor::store::{
        diesel::{
            models::{
                CommandTypeModel, CommandTypeModelMapping, ServiceLifecycleArgumentModel,
                ServiceLifecycleStatusModel, StatusTypeModel, StatusTypeModelMapping,
            },
            schema::{service_lifecycle_argument, service_lifecycle_status},
        },
        error::LifecycleStoreError,
        LifecycleService, LifecycleStatus,
    },
    LifecycleCommand, LifecycleServiceBuilder,
};
use crate::service::{FullyQualifiedServiceId, ServiceType};

pub(in crate::runtime::service::lifecycle_executor::store::diesel) trait LifecycleStoreGetServiceOperation
{
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError>;
}

impl<'a, C> LifecycleStoreGetServiceOperation for LifecycleStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<StatusTypeModelMapping>,
    StatusTypeModel: diesel::deserialize::FromSql<StatusTypeModelMapping, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<CommandTypeModelMapping>,
    CommandTypeModel: diesel::deserialize::FromSql<CommandTypeModelMapping, C::Backend>,
{
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<LifecycleService>, LifecycleStoreError> {
        self.conn.transaction::<Option<LifecycleService>, _, _>(|| {
            // Fetch the `service` entry with the matching `service_id`.
            // return None if the `service` does not exist
            let service: ServiceLifecycleStatusModel = match service_lifecycle_status::table
                .filter(service_lifecycle_status::circuit_id.eq(service_id.circuit_id().as_str()))
                .filter(service_lifecycle_status::service_id.eq(service_id.service_id().as_str()))
                .first::<ServiceLifecycleStatusModel>(self.conn)
                .optional()?
            {
                Some(service) => service,
                None => return Ok(None),
            };

            // Collect the `service_arguments` entries with the associated `circuit_id` found
            // in the `service` entry previously fetched and the provided `service_id`.
            let arguments: Vec<(String, String)> = service_lifecycle_argument::table
                .filter(service_lifecycle_argument::circuit_id.eq(service_id.circuit_id().as_str()))
                .filter(service_lifecycle_argument::service_id.eq(service_id.service_id().as_str()))
                .order(service_lifecycle_argument::position)
                .load::<ServiceLifecycleArgumentModel>(self.conn)?
                .iter()
                .map(|arg| (arg.key.to_string(), arg.value.to_string()))
                .collect();

            let return_service = LifecycleServiceBuilder::new()
                .with_service_id(service_id)
                .with_service_type(&ServiceType::new(service.service_type)?)
                .with_arguments(&arguments)
                .with_command(&LifecycleCommand::from(service.command))
                .with_status(&LifecycleStatus::from(service.status))
                .build()
                .map_err(LifecycleStoreError::InvalidState)?;

            Ok(Some(return_service))
        })
    }
}
