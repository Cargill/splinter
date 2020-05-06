// Copyright 2018-2020 Cargill Incorporated
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

use crate::network::sender::NetworkMessageSender;

use super::dispatch::{MessageSender, PeerId};

impl MessageSender<PeerId> for NetworkMessageSender {
    fn send(&self, recipient: PeerId, message: Vec<u8>) -> Result<(), (PeerId, Vec<u8>)> {
        NetworkMessageSender::send(self, recipient.into(), message)
            .map_err(|(id, msg)| (id.into(), msg))
    }
}
