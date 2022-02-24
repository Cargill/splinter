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

use super::{FullyQualifiedServiceId, IntoMessageSender, MessageConverter, MessageSender};

pub trait MessageHandler {
    type Message;

    fn handle_message(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Self::Message,
    ) -> Result<(), InternalError>;

    fn into_handler<C, R>(self, converter: C) -> IntoMessageHandler<Self, C, Self::Message, R>
    where
        Self: Sized,
        C: MessageConverter<Self::Message, R>,
    {
        IntoMessageHandler::new(self, converter)
    }
}

pub struct IntoMessageHandler<H, C, L, R> {
    inner: H,
    converter: C,
    _left: std::marker::PhantomData<L>,
    _right: std::marker::PhantomData<R>,
}

impl<H, C, L, R> IntoMessageHandler<H, C, L, R>
where
    H: MessageHandler<Message = L>,
    C: MessageConverter<L, R>,
{
    fn new(inner: H, converter: C) -> Self {
        Self {
            inner,
            converter,
            _left: std::marker::PhantomData,
            _right: std::marker::PhantomData,
        }
    }
}

impl<H, C, L, R> MessageHandler for IntoMessageHandler<H, C, L, R>
where
    H: MessageHandler<Message = L>,
    C: MessageConverter<L, R>,
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
