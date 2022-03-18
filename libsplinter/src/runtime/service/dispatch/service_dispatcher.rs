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

use crate::error::InternalError;
use crate::service::{
    FullyQualifiedServiceId, MessageHandler, MessageHandlerFactory, MessageSenderFactory,
};

use super::task::MessageHandlerTaskRunner;
use super::type_resolver::ServiceTypeResolver;

type BoxedByteMessageHandlerFactory =
    Box<dyn MessageHandlerFactory<MessageHandler = Box<dyn MessageHandler<Message = Vec<u8>>>>>;

pub struct ServiceDispatcher {
    message_handler_factories: Vec<BoxedByteMessageHandlerFactory>,
    message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    type_resolver: Box<dyn ServiceTypeResolver + Send>,
    task_runner: Box<dyn MessageHandlerTaskRunner + Send>,
}

impl ServiceDispatcher {
    pub fn new(
        message_handler_factories: Vec<BoxedByteMessageHandlerFactory>,
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
        type_resolver: Box<dyn ServiceTypeResolver + Send>,
        task_runner: Box<dyn MessageHandlerTaskRunner + Send>,
    ) -> Self {
        Self {
            message_handler_factories,
            message_sender_factory,
            type_resolver,
            task_runner,
        }
    }

    pub fn is_routable(&self, service_id: &FullyQualifiedServiceId) -> Result<bool, InternalError> {
        if let Some(service_type) = self.type_resolver.resolve_type(service_id)? {
            Ok(self.message_handler_factories.iter().any(|factory| {
                factory
                    .service_types()
                    .iter()
                    .any(|supported_type| dbg!(supported_type) == &service_type)
            }))
        } else {
            Ok(false)
        }
    }

    pub fn dispatch(
        &self,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Vec<u8>,
    ) -> Result<(), InternalError> {
        let service_type = self
            .type_resolver
            .resolve_type(&to_service)?
            .ok_or_else(|| {
                InternalError::with_message("The to_service argument has an unknown type".into())
            })?;

        let factory = self
            .message_handler_factories
            .iter()
            .find(|factory| {
                factory
                    .service_types()
                    .iter()
                    .any(|supported_type| supported_type == &service_type)
            })
            .ok_or_else(|| {
                InternalError::with_message(format!(
                    "{} services are not handled by this service dispatcher",
                    service_type
                ))
            })?;

        self.task_runner.execute(
            &**factory,
            &*self.message_sender_factory,
            to_service,
            from_service,
            message,
        )
    }
}
