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

//! Provides the "fetch circuit" operation for the `DieselAdminServiceStore`.

use diesel::prelude::*;
use std::convert::TryFrom;

use super::{list_services::AdminServiceStoreListServicesOperation, AdminServiceStoreOperations};
use crate::admin::store::{
    diesel::{
        models::{CircuitMemberModel, CircuitModel},
        schema::{circuit, circuit_member},
    },
    error::AdminServiceStoreError,
    AuthorizationType, Circuit, CircuitBuilder, DurabilityType, PersistenceType, RouteType,
    Service,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreFetchCircuitOperation {
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreFetchCircuitOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError> {
        self.conn.transaction::<Option<Circuit>, _, _>(|| {
            // Retrieve the `circuit` entry with the matching `circuit_id`
            // return None if the `circuit` does not exist
            let circuit: CircuitModel = match circuit::table
                .select(circuit::all_columns)
                .filter(circuit::circuit_id.eq(circuit_id.to_string()))
                .first::<CircuitModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Circuit"),
                    source: Box::new(err),
                })? {
                Some(circuit) => circuit,
                None => return Ok(None),
            };

            // Collecting the members of the `Circuit`
            let members: Vec<CircuitMemberModel> = circuit_member::table
                .filter(circuit_member::circuit_id.eq(circuit_id.to_string()))
                .load(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Failed to load Circuit members"),
                    source: Box::new(err),
                })?;

            // Collecting services associated with the `Circuit` using the `list_services` method,
            // which provides a list of the `Services` with the matching `circuit_id`.
            let services: Vec<Service> = self.list_services(&circuit_id)?.collect();
            let circuit_member: Vec<String> = members
                .iter()
                .map(|member| member.node_id.to_string())
                .collect();

            Ok(Some(
                CircuitBuilder::new()
                    .with_circuit_id(&circuit.circuit_id)
                    .with_roster(&services)
                    .with_members(&circuit_member)
                    .with_auth(&AuthorizationType::try_from(circuit.auth)?)
                    .with_persistence(&PersistenceType::try_from(circuit.persistence)?)
                    .with_durability(&DurabilityType::try_from(circuit.durability)?)
                    .with_routes(&RouteType::try_from(circuit.routes)?)
                    .build()
                    .map_err(|err| AdminServiceStoreError::StorageError {
                        context: String::from("Failed to build Circuit"),
                        source: Some(Box::new(err)),
                    })?,
            ))
        })
    }
}
