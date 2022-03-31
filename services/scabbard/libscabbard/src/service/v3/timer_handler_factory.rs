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

use splinter::error::{InternalError, InvalidArgumentError};
use splinter::service::{TimerHandler, TimerHandlerFactory};

use super::ScabbardMessageByteConverter;
use super::ScabbardTimerHandler;

#[derive(Clone, Default)]
pub struct ScabbardTimerHandlerFactory;

impl TimerHandlerFactory for ScabbardTimerHandlerFactory {
    type Message = Vec<u8>;

    fn new_handler(&self) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
        let timer_handler = ScabbardTimerHandler::new();
        Ok(Box::new(
            timer_handler.into_handler(ScabbardMessageByteConverter {}),
        ))
    }

    fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
        Box::new(self.clone())
    }
}

#[derive(Default)]
pub struct ScabbardTimerHandlerFactoryBuilder {}

impl ScabbardTimerHandlerFactoryBuilder {
    pub fn build(self) -> Result<ScabbardTimerHandlerFactory, InvalidArgumentError> {
        Ok(ScabbardTimerHandlerFactory {})
    }
}
