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

//! Provides the "remove service" operation for the `DieselLifecycleStore`.

use diesel::{dsl::delete, prelude::*};

use crate::error::InvalidStateError;
use crate::runtime::service::lifecycle_executor::store::{
    diesel::{
        models::ServiceLifecycleStatusModel,
        schema::{service_lifecycle_argument, service_lifecycle_status},
    },
    error::LifecycleStoreError,
};
use crate::service::FullyQualifiedServiceId;

use super::LifecycleStoreOperations;

pub(in crate::runtime::service::lifecycle_executor::store::diesel) trait LifecycleStoreRemoveServiceOperation
{
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> LifecycleStoreRemoveServiceOperation
    for LifecycleStoreOperations<'a, diesel::pg::PgConnection>
{
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            service_lifecycle_status::table
                .filter(service_lifecycle_status::circuit_id.eq(service_id.circuit_id().as_str()))
                .filter(service_lifecycle_status::service_id.eq(service_id.service_id().as_str()))
                .first::<ServiceLifecycleStatusModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    LifecycleStoreError::InvalidState(InvalidStateError::with_message(
                        String::from("Service does not exist in LifecycleStore"),
                    ))
                })?;

            delete(
                service_lifecycle_status::table
                    .filter(
                        service_lifecycle_status::circuit_id.eq(service_id.circuit_id().as_str()),
                    )
                    .filter(
                        service_lifecycle_status::service_id.eq(service_id.service_id().as_str()),
                    ),
            )
            .execute(self.conn)?;

            delete(
                service_lifecycle_argument::table
                    .filter(
                        service_lifecycle_argument::circuit_id.eq(service_id.circuit_id().as_str()),
                    )
                    .filter(
                        service_lifecycle_argument::service_id.eq(service_id.service_id().as_str()),
                    ),
            )
            .execute(self.conn)?;
            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> LifecycleStoreRemoveServiceOperation
    for LifecycleStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), LifecycleStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            service_lifecycle_status::table
                .filter(service_lifecycle_status::circuit_id.eq(service_id.circuit_id().as_str()))
                .filter(service_lifecycle_status::service_id.eq(service_id.service_id().as_str()))
                .first::<ServiceLifecycleStatusModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    LifecycleStoreError::InvalidState(InvalidStateError::with_message(
                        String::from("Service does not exist in LifecycleStore"),
                    ))
                })?;

            delete(
                service_lifecycle_status::table
                    .filter(
                        service_lifecycle_status::circuit_id.eq(service_id.circuit_id().as_str()),
                    )
                    .filter(
                        service_lifecycle_status::service_id.eq(service_id.service_id().as_str()),
                    ),
            )
            .execute(self.conn)?;

            delete(
                service_lifecycle_argument::table
                    .filter(
                        service_lifecycle_argument::circuit_id.eq(service_id.circuit_id().as_str()),
                    )
                    .filter(
                        service_lifecycle_argument::service_id.eq(service_id.service_id().as_str()),
                    ),
            )
            .execute(self.conn)?;
            Ok(())
        })
    }
}
