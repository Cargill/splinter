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

//! Provides the "remove circuit" operation for the `DieselAdminServiceStore`.

use diesel::{dsl::delete, prelude::*};

use crate::admin::store::{
    diesel::schema::{circuit, circuit_member, node_endpoint},
    error::AdminServiceStoreError,
};

use super::{get_circuit::AdminServiceStoreFetchCircuitOperation, AdminServiceStoreOperations};

pub(in crate::admin::store::diesel) trait AdminServiceStoreRemoveCircuitOperation {
    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreRemoveCircuitOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the circuit attempting to be removed exists.
            self.get_circuit(&circuit_id).and_then(|opt_circuit| {
                // Remove the `circuit` entry with the matching `circuit_id`
                // The `circuit_id` foreign key has cascade delete, meaning all related tables
                // associated to the `circuit` table via the `circuit_id` will be deleted, if the
                // corresponding `circuit` entry with the matching `circuit_id` is deleted.
                delete(circuit::table.find(&circuit_id)).execute(self.conn)?;

                // Must individually remove the circuit's members' `node_endpoint` entries, to
                // check first if the `node_id` is a member of any other circuit, and the
                // `node_endpoint` data is still valid and, therefore, should not be deleted.
                if let Some(circuit) = opt_circuit {
                    for node_id in circuit.members() {
                        // Count the amount of `circuit_member` entries with the same `node_id`. If
                        // there are still `circuit_member` entries with the associated `node_id`,
                        // or the count is not equal to 0, the `node_enpoint` should not be deleted.
                        if let Some(0) = circuit_member::table
                            .filter(circuit_member::node_id.eq(&node_id))
                            .count()
                            .first(self.conn)
                            .optional()?
                        {
                            delete(node_endpoint::table.filter(node_endpoint::node_id.eq(node_id)))
                                .execute(self.conn)?;
                        }
                    }
                }
                Ok(())
            })
        })
    }
}
