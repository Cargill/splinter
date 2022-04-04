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

#[derive(Default)]
pub struct SingleThreadedMessageHandlerTaskRunner {}

impl SingleThreadedMessageHandlerTaskRunner {
    pub fn new() -> Self {
        Self {}
    }
}

impl MessageHandlerTaskRunner for SingleThreadedMessageHandlerTaskRunner {
    fn execute(
        &self,
        message_handler_factory: &dyn MessageHandlerFactory<
            MessageHandler = Box<dyn MessageHandler<Message = Vec<u8>>>,
        >,
        sender_factory: &dyn MessageSenderFactory<Vec<u8>>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Vec<u8>,
    ) -> Result<(), InternalError> {
        let mut handler = message_handler_factory.new_handler();
        let sender = sender_factory.new_message_sender(&to_service)?;

        handler.handle_message(&*sender, to_service, from_service, message)
    }
}
