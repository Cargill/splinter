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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::{channel, Sender};

    use crate::runtime::service::SingleThreadedMessageHandlerTaskRunner;
    use crate::service::{MessageHandler, MessageSender, Routable, ServiceId, ServiceType};

    use std::collections::HashMap;

    /// Test that a given service id may be routed
    #[test]
    fn test_is_routable() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, _rx) = channel();
        let resolver = TestResolver::new([(
            FullyQualifiedServiceId::new_from_string("JgsDS-2eRnC::a000")?,
            "testtype".to_string(),
        )]);

        let service_dispatcher = ServiceDispatcher::new(
            vec![TestMessageHandlerFactory.into_boxed()],
            Box::new(TestMessageSenderFactory { tx }),
            Box::new(resolver),
            Box::new(SingleThreadedMessageHandlerTaskRunner::new()),
        );

        assert!(
            service_dispatcher.is_routable(&FullyQualifiedServiceId::new_from_string(
                "JgsDS-2eRnC::a000"
            )?)?
        );
        assert!(
            !service_dispatcher.is_routable(&FullyQualifiedServiceId::new_from_string(
                "AAAAA-BBBBB::a000"
            )?)?
        );

        Ok(())
    }

    #[test]
    fn test_dispatch() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = channel();
        let resolver = TestResolver::new([(
            FullyQualifiedServiceId::new_from_string("JgsDS-2eRnC::a000")?,
            "testtype".to_string(),
        )]);

        let service_dispatcher = ServiceDispatcher::new(
            vec![TestMessageHandlerFactory.into_boxed()],
            Box::new(TestMessageSenderFactory { tx }),
            Box::new(resolver),
            Box::new(SingleThreadedMessageHandlerTaskRunner::new()),
        );

        let to_service = FullyQualifiedServiceId::new_from_string("JgsDS-2eRnC::a000")?;
        let from_service = FullyQualifiedServiceId::new_from_string("AAAAA-BBBBB::a000")?;

        service_dispatcher.dispatch(to_service.clone(), from_service.clone(), b"hello".to_vec())?;

        let (scope, to, out_msg) = rx.recv()?;

        assert_eq!(scope, to_service);
        assert_eq!(&to, from_service.service_id());
        assert_eq!(&out_msg, b"hello;out");

        Ok(())
    }

    struct TestResolver {
        mapping: HashMap<FullyQualifiedServiceId, String>,
    }

    impl TestResolver {
        fn new<const N: usize>(entries: [(FullyQualifiedServiceId, String); N]) -> Self {
            Self {
                mapping: entries.into(),
            }
        }
    }

    impl ServiceTypeResolver for TestResolver {
        fn resolve_type(
            &self,
            service_id: &FullyQualifiedServiceId,
        ) -> Result<Option<ServiceType>, InternalError> {
            Ok(self
                .mapping
                .get(service_id)
                .cloned()
                .map(|s| ServiceType::new(s).unwrap()))
        }
    }

    #[derive(Clone)]
    struct TestMessageHandlerFactory;

    impl MessageHandlerFactory for TestMessageHandlerFactory {
        type MessageHandler = TestMessageHandler;

        fn new_handler(&self) -> Self::MessageHandler {
            TestMessageHandler
        }

        fn clone_boxed(
            &self,
        ) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
            Box::new(self.clone())
        }
    }

    const TEST_TYPES: &'static [ServiceType] = &[ServiceType::new_static("testtype")];

    impl Routable for TestMessageHandlerFactory {
        fn service_types(&self) -> &[ServiceType] {
            TEST_TYPES
        }
    }

    struct TestMessageHandler;

    impl MessageHandler for TestMessageHandler {
        type Message = Vec<u8>;

        fn handle_message(
            &mut self,
            sender: &dyn MessageSender<Self::Message>,
            _to_service: FullyQualifiedServiceId,
            from_service: FullyQualifiedServiceId,
            message: Self::Message,
        ) -> Result<(), InternalError> {
            let mut msg = message;
            msg.extend(b";out");

            sender.send(from_service.service_id(), msg)
        }
    }

    struct TestMessageSender {
        scope: FullyQualifiedServiceId,
        tx: Sender<(FullyQualifiedServiceId, ServiceId, Vec<u8>)>,
    }

    impl MessageSender<Vec<u8>> for TestMessageSender {
        fn send(&self, to_service: &ServiceId, message: Vec<u8>) -> Result<(), InternalError> {
            self.tx
                .send((self.scope.clone(), to_service.clone(), message))
                .map_err(|_| InternalError::with_message("Receiver dropped".into()))
        }
    }

    #[derive(Clone)]
    struct TestMessageSenderFactory {
        tx: Sender<(FullyQualifiedServiceId, ServiceId, Vec<u8>)>,
    }

    impl MessageSenderFactory<Vec<u8>> for TestMessageSenderFactory {
        fn new_message_sender(
            &self,
            from_service: &FullyQualifiedServiceId,
        ) -> Result<Box<dyn MessageSender<Vec<u8>>>, InternalError> {
            Ok(Box::new(TestMessageSender {
                scope: from_service.clone(),
                tx: self.tx.clone(),
            }))
        }

        fn clone_boxed(&self) -> Box<dyn MessageSenderFactory<Vec<u8>>> {
            Box::new(self.clone())
        }
    }
}
