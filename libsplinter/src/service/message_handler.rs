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

//! Contains `MessageHandler` trait.

use crate::error::InternalError;

use super::{FullyQualifiedServiceId, IntoMessageSender, MessageConverter, MessageSender};

/// Handles an inbound message for a service implementation.
///
/// A `MessageHandler` is run when there is work to be performed from an incoming message
/// and together with `TimerHandler`, will contain most of the logic of the service. When run, a
/// `MessageSender` is provided a sender, the sender and recipient of the message and the message.
pub trait MessageHandler {
    type Message;

    /// Handle an incoming message
    ///
    /// # Arguments
    ///
    /// * `sender` - The sender for any messages that need to be sent
    /// * `to_service` - The service the message is for
    /// * `from_service` - The service that sent the message
    /// * `message` - The message to be handled
    fn handle_message(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Self::Message,
    ) -> Result<(), InternalError>;

    fn into_handler<C, R>(self, converter: C) -> IntoMessageHandler<Self, C, R>
    where
        Self: Sized,
        C: MessageConverter<Self::Message, R>,
    {
        IntoMessageHandler::new(self, converter)
    }
}

pub struct IntoMessageHandler<H, C, R> {
    inner: H,
    converter: C,
    _right: std::marker::PhantomData<R>,
}

impl<H, C, R> IntoMessageHandler<H, C, R>
where
    H: MessageHandler,
    C: MessageConverter<<H as MessageHandler>::Message, R>,
{
    fn new(inner: H, converter: C) -> Self {
        Self {
            inner,
            converter,
            _right: std::marker::PhantomData,
        }
    }
}

impl<H, C, R> MessageHandler for IntoMessageHandler<H, C, R>
where
    H: MessageHandler,
    C: MessageConverter<<H as MessageHandler>::Message, R>,
{
    type Message = R;

    fn handle_message(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Self::Message,
    ) -> Result<(), InternalError> {
        let left_message = self.converter.to_left(message)?;
        let left_sender = IntoMessageSender::new(sender, &self.converter);
        self.inner
            .handle_message(&left_sender, to_service, from_service, left_message)
    }
}

impl<T> MessageHandler for Box<dyn MessageHandler<Message = T>> {
    type Message = T;

    fn handle_message(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Self::Message,
    ) -> Result<(), InternalError> {
        (&mut **self).handle_message(sender, to_service, from_service, message)
    }
}
