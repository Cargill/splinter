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

//! Provides the "remove proposal" operation for the `DieselAdminServiceStore`.

use diesel::{
    dsl::delete,
    prelude::*,
    sql_types::{Binary, Text},
};

use crate::admin::store::{
    diesel::{
        models::{CircuitProposalModel, ProposedCircuitModel, VoteRecordModel},
        schema::circuit_proposal,
    },
    error::AdminServiceStoreError,
};

use super::{get_proposal::AdminServiceStoreFetchProposalOperation, AdminServiceStoreOperations};

pub(in crate::admin::store::diesel) trait AdminServiceStoreRemoveProposalOperation {
    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreRemoveProposalOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    CircuitProposalModel: diesel::Queryable<(Text, Text, Text, Binary, Text), C::Backend>,
    ProposedCircuitModel:
        diesel::Queryable<(Text, Text, Text, Text, Text, Text, Binary, Text), C::Backend>,
    VoteRecordModel: diesel::Queryable<(Text, Binary, Text, Text), C::Backend>,
{
    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the `proposal` being removed exists
            self.get_proposal(&proposal_id).and_then(|_| {
                // Remove the `proposal` entry with the matching `proposal_id`, which is represented
                // in the `circuit_proposal` by the `circuit_id`.
                // The `circuit_id` foreign key has cascade delete, meaning all related tables
                // associated to the `circuit` table via the `circuit_id` will be deleted, if the
                // corresponding `circuit` entry with the matching `circuit_id` is deleted.
                delete(circuit_proposal::table.find(&proposal_id)).execute(self.conn)?;
                Ok(())
            })
        })
    }
}
