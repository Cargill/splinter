// Copyright 2018-2022 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use splinter::service::MessageHandlerFactory;

use super::message_handler::ScabbardMessageHandler;

#[derive(Clone, Default)]
pub struct ScabbardMessageHandlerFactory {}

impl ScabbardMessageHandlerFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl MessageHandlerFactory for ScabbardMessageHandlerFactory {
    type MessageHandler = ScabbardMessageHandler;

    fn new_handler(&self) -> Self::MessageHandler {
        ScabbardMessageHandler::new()
    }

    fn clone_boxed(&self) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
        Box::new(self.clone())
    }
}
