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

use super::{message_handler::IntoMessageHandler, MessageConverter, MessageHandler};

pub trait MessageHandlerFactory: Send {
    type MessageHandler: MessageHandler;

    fn new_handler(&self) -> Self::MessageHandler;

    fn clone_boxed(&self) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>>;

    fn into_factory<C, R>(self, converter: C) -> IntoMessageHandlerFactory<Self, C, R>
    where
        Self: Sized,
        C: MessageConverter<<Self::MessageHandler as MessageHandler>::Message, R> + Send + Clone,
    {
        IntoMessageHandlerFactory::new(self, converter)
    }

    fn into_boxed(
        self,
    ) -> Box<
        dyn MessageHandlerFactory<
            MessageHandler = Box<
                dyn MessageHandler<Message = <Self::MessageHandler as MessageHandler>::Message>,
            >,
        >,
    >
    where
        Self: Clone + Sized + 'static,
    {
        Box::new(BoxedMessageHandlerFactory::new(self))
    }
}

impl<H> Clone for Box<dyn MessageHandlerFactory<MessageHandler = H>>
where
    H: MessageHandler,
{
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

pub struct IntoMessageHandlerFactory<F, C, R> {
    inner: F,
    converter: C,
    _right: std::marker::PhantomData<R>,
}

impl<F, C, R> IntoMessageHandlerFactory<F, C, R>
where
    F: MessageHandlerFactory,
    C: MessageConverter<
            <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
            R,
        > + Send
        + Clone,
{
    fn new(inner: F, converter: C) -> Self {
        Self {
            inner,
            converter,
            _right: std::marker::PhantomData,
        }
    }
}

impl<F, C, R> MessageHandlerFactory for IntoMessageHandlerFactory<F, C, R>
where
    F: MessageHandlerFactory + Clone + 'static,
    C: MessageConverter<
            <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
            R,
        > + Send
        + Clone
        + 'static,
    R: 'static,
{
    type MessageHandler = IntoMessageHandler<
        <F as MessageHandlerFactory>::MessageHandler,
        C,
        <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
        R,
    >;

    fn new_handler(&self) -> Self::MessageHandler {
        let handler = self.inner.new_handler();
        handler.into_handler(self.converter.clone())
    }

    fn clone_boxed(&self) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
        Box::new(self.clone())
    }
}

impl<F, C, R> Clone for IntoMessageHandlerFactory<F, C, R>
where
    F: MessageHandlerFactory + Clone + 'static,
    C: MessageConverter<
            <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
            R,
        > + Send
        + Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            converter: self.converter.clone(),
            _right: std::marker::PhantomData,
        }
    }
}

// This is safe to implement as the actual values (F, C) in this struct are Send, but the
// PhantomType values are not, but are required for the generics to operate correctly
unsafe impl<F, C, R> Send for IntoMessageHandlerFactory<F, C, R>
where
    F: MessageHandlerFactory + Clone,
    C: MessageConverter<
            <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
            R,
        > + Send
        + Clone,
{
}

struct BoxedMessageHandlerFactory<F> {
    inner: F,
}

impl<F> BoxedMessageHandlerFactory<F>
where
    F: MessageHandlerFactory + 'static,
{
    fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl<F> MessageHandlerFactory for BoxedMessageHandlerFactory<F>
where
    F: MessageHandlerFactory + Clone + 'static,
{
    type MessageHandler = Box<
        dyn MessageHandler<
            Message = <<F as MessageHandlerFactory>::MessageHandler as MessageHandler>::Message,
        >,
    >;

    fn new_handler(&self) -> Self::MessageHandler {
        let handler = self.inner.new_handler();
        Box::new(handler)
    }

    fn clone_boxed(&self) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
        Box::new(Self {
            inner: self.inner.clone(),
        })
    }
}
