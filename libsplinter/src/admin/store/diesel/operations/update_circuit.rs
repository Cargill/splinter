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

//! Provides the "update circuit" operation for the `DieselAdminServiceStore`.

use diesel::{
    dsl::{delete, insert_into, update},
    prelude::*,
};

use super::AdminServiceStoreOperations;
use crate::admin::store::{
    diesel::{
        models::{
            CircuitMemberModel, CircuitModel, ServiceAllowedNodeModel, ServiceArgumentModel,
            ServiceModel,
        },
        schema::{circuit, circuit_member, service, service_allowed_node, service_argument},
    },
    error::AdminServiceStoreError,
    Circuit,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreUpdateCircuitOperation {
    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> AdminServiceStoreUpdateCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the `circuit` entry to be updated exists
            circuit::table
                .filter(circuit::circuit_id.eq(circuit.circuit_id()))
                .first::<CircuitModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Circuit"),
                    source: Box::new(err),
                })?
                .ok_or_else(|| {
                    AdminServiceStoreError::NotFoundError(String::from(
                        "Circuit does not exist in AdminServiceStore",
                    ))
                })?;

            // Update existing `Circuit`
            let circuit_model = CircuitModel::from(&circuit);
            update(circuit::table.find(circuit.circuit_id()))
                .set((
                    circuit::auth.eq(circuit_model.auth),
                    circuit::persistence.eq(circuit_model.persistence),
                    circuit::durability.eq(circuit_model.durability),
                    circuit::routes.eq(circuit_model.routes),
                    circuit::circuit_management_type.eq(circuit_model.circuit_management_type),
                ))
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to update Circuit"),
                    source: Box::new(err),
                })?;
            // Delete existing data associated with the `Circuit`
            delete(service::table.filter(service::circuit_id.eq(circuit.circuit_id())))
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Failed to remove old Services"),
                    source: Box::new(err),
                })?;
            delete(
                service_allowed_node::table
                    .filter(service_allowed_node::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)
            .map_err(|err| AdminServiceStoreError::QueryError {
                context: String::from("Failed to remove old Services' allowed nodes"),
                source: Box::new(err),
            })?;
            delete(
                service_argument::table
                    .filter(service_argument::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)
            .map_err(|err| AdminServiceStoreError::QueryError {
                context: String::from("Failed to remove old Service arguments"),
                source: Box::new(err),
            })?;
            // Insert new data associate with the `Circuit`
            let services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services"),
                    source: Box::new(err),
                })?;
            let service_allowed_node: Vec<ServiceAllowedNodeModel> = Vec::from(&circuit);
            insert_into(service_allowed_node::table)
                .values(&service_allowed_node)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services' allowed nodes"),
                    source: Box::new(err),
                })?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Service arguments"),
                    source: Box::new(err),
                })?;
            let circuit_member: Vec<CircuitMemberModel> = Vec::from(&circuit);
            insert_into(circuit_member::table)
                .values(circuit_member)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit members"),
                    source: Box::new(err),
                })?;
            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreUpdateCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the `circuit` entry to be updated exists
            circuit::table
                .filter(circuit::circuit_id.eq(circuit.circuit_id()))
                .first::<CircuitModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Circuit"),
                    source: Box::new(err),
                })?
                .ok_or_else(|| {
                    AdminServiceStoreError::NotFoundError(String::from(
                        "Circuit does not exist in AdminServiceStore",
                    ))
                })?;

            // Update existing `Circuit`
            let circuit_model = CircuitModel::from(&circuit);
            update(circuit::table.find(circuit.circuit_id()))
                .set((
                    circuit::auth.eq(circuit_model.auth),
                    circuit::persistence.eq(circuit_model.persistence),
                    circuit::durability.eq(circuit_model.durability),
                    circuit::routes.eq(circuit_model.routes),
                    circuit::circuit_management_type.eq(circuit_model.circuit_management_type),
                ))
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to update Circuit"),
                    source: Box::new(err),
                })?;
            // Delete existing data associated with the `Circuit`
            delete(service::table.filter(service::circuit_id.eq(circuit.circuit_id())))
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Failed to remove old Services"),
                    source: Box::new(err),
                })?;
            delete(
                service_allowed_node::table
                    .filter(service_allowed_node::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)
            .map_err(|err| AdminServiceStoreError::QueryError {
                context: String::from("Failed to remove old Services' allowed nodes"),
                source: Box::new(err),
            })?;
            delete(
                service_argument::table
                    .filter(service_argument::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)
            .map_err(|err| AdminServiceStoreError::QueryError {
                context: String::from("Failed to remove old Service arguments"),
                source: Box::new(err),
            })?;
            // Insert new `Circuit` data
            let services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services"),
                    source: Box::new(err),
                })?;
            let service_allowed_node: Vec<ServiceAllowedNodeModel> = Vec::from(&circuit);
            insert_into(service_allowed_node::table)
                .values(&service_allowed_node)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services' allowed nodes"),
                    source: Box::new(err),
                })?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Service arguments"),
                    source: Box::new(err),
                })?;
            let circuit_member: Vec<CircuitMemberModel> = Vec::from(&circuit);
            insert_into(circuit_member::table)
                .values(circuit_member)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit members"),
                    source: Box::new(err),
                })?;
            Ok(())
        })
    }
}
