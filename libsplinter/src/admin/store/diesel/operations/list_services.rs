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

//! Provides the "list services" operation for the `DieselAdminServiceStore`.

use std::collections::HashMap;

use diesel::prelude::*;

use crate::admin::store::{
    diesel::{
        models::{ServiceArgumentModel, ServiceModel},
        schema::{service, service_argument},
    },
    error::AdminServiceStoreError,
    Service, ServiceBuilder,
};

use super::AdminServiceStoreOperations;

pub(in crate::admin::store::diesel) trait AdminServiceStoreListServicesOperation {
    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreListServicesOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError> {
        // Create HashMap of `service_id` to a `ServiceBuilder` to collect `Service` information
        let mut services: HashMap<String, ServiceBuilder> = HashMap::new();
        // Create HashMap of `service_id` to the associated argument values
        let mut arguments_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
        // Collect all 'service' entries and associated data using `inner_join`, as each `service`
        // entry has a one-to-many relationship to `service_argument`.
        for (service, opt_arg) in service::table
            // Filter retrieved 'service' entries by the provided `circuit_id`
            .filter(service::circuit_id.eq(&circuit_id))
            // The `service` table has a one-to-many relationship with the `service_argument` table.
            // The `inner_join` will retrieve the `service` and all `service_argument` entries
            // with the matching `circuit_id` and `service_id`.
            .left_join(
                service_argument::table.on(service::circuit_id
                    .eq(service_argument::circuit_id)
                    .and(service::service_id.eq(service_argument::service_id))),
            )
            // Making the `service_argument` data `nullable`, removes
            // the requirement for different numbers of each to be returned with, or without
            // an associated entry from the other table.
            .select((
                service::all_columns,
                service_argument::all_columns.nullable(),
            ))
            .load::<(ServiceModel, Option<ServiceArgumentModel>)>(self.conn)?
        {
            if let Some(arg_model) = opt_arg {
                if let Some(args) = arguments_map.get_mut(&service.service_id) {
                    args.push((arg_model.key.to_string(), arg_model.value.to_string()));
                } else {
                    arguments_map.insert(
                        service.service_id.to_string(),
                        vec![(arg_model.key.to_string(), arg_model.value.to_string())],
                    );
                }
            }
            // Insert new `ServiceBuilder` if it does not already exist
            if !services.contains_key(&service.service_id) {
                services.insert(
                    service.service_id.to_string(),
                    ServiceBuilder::new()
                        .with_service_id(&service.service_id)
                        .with_service_type(&service.service_type)
                        .with_node_id(&service.node_id),
                );
            }
        }

        let ret_services: Vec<Service> = services
            .into_iter()
            .map(|(id, mut builder)| {
                if let Some(args) = arguments_map.get(&id) {
                    builder = builder.with_arguments(&args);
                }
                builder
                    .build()
                    .map_err(AdminServiceStoreError::InvalidStateError)
            })
            .collect::<Result<Vec<Service>, AdminServiceStoreError>>()?;

        Ok(Box::new(ret_services.into_iter()))
    }
}
