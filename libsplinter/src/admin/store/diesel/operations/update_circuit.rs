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
        models::{CircuitMemberModel, CircuitModel, ServiceArgumentModel, ServiceModel},
        schema::{circuit, circuit_member, service, service_argument},
    },
    error::AdminServiceStoreError,
    Circuit,
};
use crate::error::InvalidStateError;

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
                .optional()?
                .ok_or_else(|| {
                    AdminServiceStoreError::InvalidStateError(InvalidStateError::with_message(
                        String::from("Circuit does not exist in AdminServiceStore"),
                    ))
                })?;

            // Update existing `Circuit`
            let circuit_model = CircuitModel::from(&circuit);
            update(circuit::table.find(circuit.circuit_id()))
                .set((
                    circuit::authorization_type.eq(circuit_model.authorization_type),
                    circuit::persistence.eq(circuit_model.persistence),
                    circuit::durability.eq(circuit_model.durability),
                    circuit::routes.eq(circuit_model.routes),
                    circuit::circuit_management_type.eq(circuit_model.circuit_management_type),
                ))
                .execute(self.conn)?;
            // Delete existing data associated with the `Circuit`
            delete(service::table.filter(service::circuit_id.eq(circuit.circuit_id())))
                .execute(self.conn)?;
            delete(
                service_argument::table
                    .filter(service_argument::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)?;
            // Insert new data associate with the `Circuit`
            let services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&services)
                .execute(self.conn)?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)?;
            let circuit_member: Vec<CircuitMemberModel> = Vec::from(&circuit);
            insert_into(circuit_member::table)
                .values(circuit_member)
                .execute(self.conn)?;
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
                .optional()?
                .ok_or_else(|| {
                    AdminServiceStoreError::InvalidStateError(InvalidStateError::with_message(
                        String::from("Circuit does not exist in AdminServiceStore"),
                    ))
                })?;

            // Update existing `Circuit`
            let circuit_model = CircuitModel::from(&circuit);
            update(circuit::table.find(circuit.circuit_id()))
                .set((
                    circuit::authorization_type.eq(circuit_model.authorization_type),
                    circuit::persistence.eq(circuit_model.persistence),
                    circuit::durability.eq(circuit_model.durability),
                    circuit::routes.eq(circuit_model.routes),
                    circuit::circuit_management_type.eq(circuit_model.circuit_management_type),
                ))
                .execute(self.conn)?;
            // Delete existing data associated with the `Circuit`
            delete(service::table.filter(service::circuit_id.eq(circuit.circuit_id())))
                .execute(self.conn)?;
            delete(
                service_argument::table
                    .filter(service_argument::circuit_id.eq(circuit.circuit_id())),
            )
            .execute(self.conn)?;
            // Insert new data associate with the `Circuit`
            let services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&services)
                .execute(self.conn)?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)?;
            let circuit_member: Vec<CircuitMemberModel> = Vec::from(&circuit);
            insert_into(circuit_member::table)
                .values(circuit_member)
                .execute(self.conn)?;
            Ok(())
        })
    }
}
