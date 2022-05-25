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
use diesel::{dsl::insert_into, prelude::*};
use splinter::error::{ConstraintViolationError, ConstraintViolationType};

use crate::store::scabbard_store::diesel::{
    models::{ScabbardPeerModel, ScabbardServiceModel},
    schema::{scabbard_peer, scabbard_service},
};
use crate::store::scabbard_store::service::ScabbardService;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "add_service";

pub(in crate::store::scabbard_store::diesel) trait AddServiceOperation {
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddServiceOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if the service already exists
            if scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", service.service_id())))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .is_some()
            {
                return Err(ScabbardStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }

            insert_into(scabbard_service::table)
                .values(vec![ScabbardServiceModel::from(&service)])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            if !service.peers().is_empty() {
                insert_into(scabbard_peer::table)
                    .values(Vec::<ScabbardPeerModel>::from(&service))
                    .execute(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;
            }
            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddServiceOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if the service already exists
            if scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", service.service_id())))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .is_some()
            {
                return Err(ScabbardStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }

            insert_into(scabbard_service::table)
                .values(vec![ScabbardServiceModel::from(&service)])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            if !service.peers().is_empty() {
                insert_into(scabbard_peer::table)
                    .values(Vec::<ScabbardPeerModel>::from(&service))
                    .execute(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;
            }
            Ok(())
        })
    }
}
