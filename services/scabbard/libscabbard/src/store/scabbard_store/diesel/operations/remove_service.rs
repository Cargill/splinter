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

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{dsl::delete, prelude::*};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::operations::get_service::GetServiceOperation;
use crate::store::scabbard_store::diesel::schema::{
    consensus_2pc_consensus_coordinator_context, consensus_2pc_participant_context, scabbard_peer,
    scabbard_service, scabbard_v3_commit_history,
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait RemoveServiceOperation {
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> RemoveServiceOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // Verify the service attempting to be removed exists.
            self.get_service(service_id).and_then(|_| {
                let service_id = service_id.to_string();
                // delete service and peers
                delete(scabbard_service::table.find(&service_id)).execute(self.conn)?;
                delete(scabbard_peer::table.filter(scabbard_peer::service_id.eq(&service_id)))
                    .execute(self.conn)?;

                // delete all commit history for the service
                delete(
                    scabbard_v3_commit_history::table
                        .filter(scabbard_v3_commit_history::service_id.eq(&service_id)),
                )
                .execute(self.conn)?;

                // delete all consensus state associated with the services
                //
                // delete cascade will remove the remaining associated consensus components
                delete(consensus_2pc_consensus_coordinator_context::table.filter(
                    consensus_2pc_consensus_coordinator_context::service_id.eq(&service_id),
                ))
                .execute(self.conn)?;

                delete(
                    consensus_2pc_participant_context::table
                        .filter(consensus_2pc_participant_context::service_id.eq(&service_id)),
                )
                .execute(self.conn)?;

                Ok(())
            })
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> RemoveServiceOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        {
            self.conn.transaction::<_, _, _>(|| {
                // Verify the service attempting to be removed exists.
                self.get_service(service_id).and_then(|_| {
                    let service_id = service_id.to_string();
                    // delete service and peers
                    delete(scabbard_service::table.find(&service_id)).execute(self.conn)?;
                    delete(scabbard_peer::table.filter(scabbard_peer::service_id.eq(&service_id)))
                        .execute(self.conn)?;

                    // delete all commit history for the service
                    delete(
                        scabbard_v3_commit_history::table
                            .filter(scabbard_v3_commit_history::service_id.eq(&service_id)),
                    )
                    .execute(self.conn)?;

                    // delete all consensus state associated with the services
                    //
                    // delete cascade will remove the remaining associated consensus components
                    delete(consensus_2pc_consensus_coordinator_context::table.filter(
                        consensus_2pc_consensus_coordinator_context::service_id.eq(&service_id),
                    ))
                    .execute(self.conn)?;

                    delete(
                        consensus_2pc_participant_context::table
                            .filter(consensus_2pc_participant_context::service_id.eq(&service_id)),
                    )
                    .execute(self.conn)?;

                    Ok(())
                })
            })
        }
    }
}
