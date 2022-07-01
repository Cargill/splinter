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
use crate::store::scabbard_store::diesel::schema::scabbard_service;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "remove_service";

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
                let circuit_id = service_id.circuit_id().to_string();
                let service_id = service_id.service_id().to_string();
                // delete service and peers
                delete(scabbard_service::table.find((&circuit_id, &service_id)))
                    .execute(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;

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
                    let circuit_id = service_id.circuit_id().to_string();
                    let service_id = service_id.service_id().to_string();
                    // delete service and peers
                    delete(scabbard_service::table.find((&circuit_id, &service_id)))
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                    Ok(())
                })
            })
        }
    }
}
