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
use diesel::{dsl::insert_into, prelude::*, result::Error::NotFound};
use splinter::{error::InternalError, service::FullyQualifiedServiceId};

use crate::service::EchoArguments;
use crate::store::diesel::{
    models::{EchoPeer, EchoService, EchoServiceStatusModel},
    schema::{echo_peers, echo_services},
};

use super::EchoStoreOperations;

pub(in crate::store::diesel) trait AddServiceOperation {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddServiceOperation for EchoStoreOperations<'a, SqliteConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            if echo_services::table
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .first::<EchoService>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .is_some()
            {
                return Err(InternalError::with_message(format!(
                    "Failed to add echo service, service ID {} already exists",
                    service
                )));
            }

            let new_service = EchoService {
                service_id: format!("{}", service),
                frequency: Some(arguments.frequency().as_millis() as i64),
                jitter: Some(arguments.jitter().as_millis() as i64),
                error_rate: Some(arguments.error_rate()),
                status: EchoServiceStatusModel::Prepared,
            };

            insert_into(echo_services::table)
                .values(vec![new_service])
                .execute(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            if !arguments.peers().is_empty() {
                let mut peers = Vec::new();
                for peer in arguments.peers() {
                    let echo_peer = EchoPeer {
                        service_id: format!("{}", service),
                        peer_service_id: Some(format!("{}", peer)),
                    };
                    peers.push(echo_peer);
                }

                insert_into(echo_peers::table)
                    .values(peers)
                    .execute(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
            }

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddServiceOperation for EchoStoreOperations<'a, PgConnection> {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError> {
        self.conn.transaction::<_, _, _>(|| {
            if echo_services::table
                .filter(echo_services::service_id.eq(format!("{}", service)))
                .first::<EchoService>(self.conn)
                .map(Some)
                .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .is_some()
            {
                return Err(InternalError::with_message(format!(
                    "Failed to add echo service, service ID {} already exists",
                    service
                )));
            }

            let new_service = EchoService {
                service_id: format!("{}", service),
                frequency: Some(arguments.frequency().as_millis() as i64),
                jitter: Some(arguments.jitter().as_millis() as i64),
                error_rate: Some(arguments.error_rate()),
                status: EchoServiceStatusModel::Prepared,
            };

            insert_into(echo_services::table)
                .values(vec![new_service])
                .execute(self.conn)
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            if !arguments.peers().is_empty() {
                let mut peers = Vec::new();
                for peer in arguments.peers() {
                    let echo_peer = EchoPeer {
                        service_id: format!("{}", service),
                        peer_service_id: Some(format!("{}", peer)),
                    };
                    peers.push(echo_peer);
                }

                insert_into(echo_peers::table)
                    .values(peers)
                    .execute(self.conn)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
            }

            Ok(())
        })
    }
}
