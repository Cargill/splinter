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
use crate::service::FullyQualifiedServiceId;

use super::{IntoMessageSender, MessageConverter, MessageSender};

pub trait TimerHandler {
    type Message;

    fn handle_timer(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        service: FullyQualifiedServiceId,
    ) -> Result<(), InternalError>;

    fn into_handler<C, R>(self, converter: C) -> IntoTimerHandler<Self, C, Self::Message, R>
    where
        Self: Sized,
        C: MessageConverter<Self::Message, R>,
    {
        IntoTimerHandler::new(self, converter)
    }
}

pub struct IntoTimerHandler<H, C, L, R> {
    inner: H,
    converter: C,
    _left: std::marker::PhantomData<L>,
    _right: std::marker::PhantomData<R>,
}

impl<H, C, L, R> IntoTimerHandler<H, C, L, R>
where
    H: TimerHandler<Message = L>,
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

impl<H, C, L, R> TimerHandler for IntoTimerHandler<H, C, L, R>
where
    H: TimerHandler<Message = L>,
    C: MessageConverter<L, R>,
{
    type Message = R;

    fn handle_timer(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        service: FullyQualifiedServiceId,
    ) -> Result<(), InternalError> {
        let left_sender = IntoMessageSender::new(sender, &self.converter);
        self.inner.handle_timer(&left_sender, service)
    }
}
