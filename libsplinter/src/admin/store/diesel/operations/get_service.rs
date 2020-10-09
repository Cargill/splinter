// Copyright 2018-2020 Cargill Incorporated
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

//! Provides the "fetch service" operation for the `DieselAdminServiceStore`.

use diesel::prelude::*;

use super::AdminServiceStoreOperations;
use crate::admin::store::{
    diesel::{
        models::{ServiceArgumentModel, ServiceModel},
        schema::{service, service_argument},
    },
    error::AdminServiceStoreError,
    Service, ServiceBuilder, ServiceId,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreFetchServiceOperation {
    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreFetchServiceOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError> {
        self.conn.transaction::<Option<Service>, _, _>(|| {
            // Fetch the `service` entry with the matching `service_id`.
            let service: ServiceModel = service::table
                .filter(service::circuit_id.eq(&service_id.circuit_id))
                .filter(service::service_id.eq(&service_id.service_id))
                .first::<ServiceModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Service"),
                    source: Box::new(err),
                })?
                .ok_or_else(|| {
                    AdminServiceStoreError::NotFoundError(String::from(
                        "Service does not exist in AdminServiceStore",
                    ))
                })?;

            // Collect the `service_argument` entries with the associated `circuit_id` found
            // in the `service` entry previously fetched and the provided `service_id`.
            let arguments: Vec<(String, String)> = service_argument::table
                .filter(service_argument::circuit_id.eq(&service_id.circuit_id))
                .filter(service_argument::service_id.eq(&service_id.service_id))
                .load::<ServiceArgumentModel>(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Service arguments"),
                    source: Box::new(err),
                })?
                .iter()
                .map(|arg| (arg.key.to_string(), arg.value.to_string()))
                .collect();

            let return_service = ServiceBuilder::new()
                .with_service_id(&service.service_id)
                .with_service_type(&service.service_type)
                .with_arguments(&arguments)
                .with_node_id(&service.node_id)
                .build()
                .map_err(|err| AdminServiceStoreError::StorageError {
                    context: String::from("Failed to build Service"),
                    source: Some(Box::new(err)),
                })?;
            Ok(Some(return_service))
        })
    }
}
