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
use std::sync::mpsc::Sender;

pub struct DeferredSend<M> {
    sender: Sender<M>,
    msg: Option<M>,
}

impl<M> DeferredSend<M> {
    pub fn new(sender: Sender<M>, msg: M) -> Self {
        DeferredSend {
            sender,
            msg: Some(msg),
        }
    }
}

impl<M> Drop for DeferredSend<M> {
    fn drop(&mut self) {
        if let Some(msg) = self.msg.take() {
            if self.sender.send(msg).is_err() {
                error!("Unable to send message for deferred send")
            }
        }
    }
}
