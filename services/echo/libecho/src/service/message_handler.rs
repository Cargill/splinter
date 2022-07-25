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

use log::info;
use splinter::{
    error::InternalError,
    service::{FullyQualifiedServiceId, MessageHandler, MessageSender},
};
use std::convert::TryFrom;
use std::time::SystemTime;

use super::EchoMessage;

use crate::service::RequestStatus;
use crate::store::EchoStore;

pub struct EchoMessageHandler {
    store: Box<dyn EchoStore>,
}

impl EchoMessageHandler {
    pub fn new(store: Box<dyn EchoStore>) -> Self {
        EchoMessageHandler { store }
    }
}

impl MessageHandler for EchoMessageHandler {
    type Message = EchoMessage;

    fn handle_message(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Self::Message,
    ) -> Result<(), InternalError> {
        match message {
            EchoMessage::Request {
                message,
                correlation_id,
            } => {
                info!("[service:{}] [from:{}] [id:{}] received echo request, sending echo response: \"{}\"", to_service, from_service, correlation_id, message);
                sender.send(
                    from_service.service_id(),
                    EchoMessage::Response {
                        message,
                        correlation_id,
                    },
                )
            }
            EchoMessage::Response {
                message,
                correlation_id,
            } => {
                info!(
                    "[service:{}] [from:{}] [id:{}] received echo response: \"{}\"",
                    to_service, from_service, correlation_id, message
                );
                let ack_at = SystemTime::now();
                let ack_at = i64::try_from(
                    ack_at
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                self.store.update_request_ack(
                    &to_service,
                    correlation_id as i64,
                    RequestStatus::Sent,
                    Some(ack_at),
                )?;
                Ok(())
            }
        }
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod test {
    use super::*;
    use splinter::service::MessageConverter;

    #[cfg(feature = "diesel_migrations")]
    use crate::migrations::run_sqlite_migrations;
    use crate::store::DieselEchoStore;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    struct EchoMessageToBytesConverter {}

    impl MessageConverter<EchoMessage, Vec<u8>> for EchoMessageToBytesConverter {
        fn to_right(&self, _left: EchoMessage) -> Result<Vec<u8>, InternalError> {
            unimplemented!()
        }
        fn to_left(&self, _right: Vec<u8>) -> Result<EchoMessage, InternalError> {
            unimplemented!()
        }
    }

    #[test]
    fn test_it() {
        let mut list: Vec<Box<dyn MessageHandler<Message = Vec<u8>>>> = Vec::new();

        let pool = create_connection_pool_and_migrate();
        let store = DieselEchoStore::new(pool);

        let converter = EchoMessageToBytesConverter {};
        let handler = EchoMessageHandler::new(Box::new(store));
        let byte_handler = handler.into_handler(converter);

        list.push(Box::new(byte_handler));
    }

    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
