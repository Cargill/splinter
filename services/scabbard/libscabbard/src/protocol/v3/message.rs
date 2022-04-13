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

use crate::protos::prelude::*;
use crate::protos::scabbard_v3;

#[derive(Debug)]
pub enum ScabbardMessage {
    ConsensusMessage(Vec<u8>),
}

impl FromProto<scabbard_v3::ScabbardMessageV3> for ScabbardMessage {
    fn from_proto(
        mut source: scabbard_v3::ScabbardMessageV3,
    ) -> Result<Self, ProtoConversionError> {
        use scabbard_v3::ScabbardMessageV3_Type::*;
        match source.get_message_type() {
            CONSENSUS_MESSAGE => Ok(ScabbardMessage::ConsensusMessage(
                source.take_consensus_message(),
            )),
            UNSET => Err(ProtoConversionError::InvalidTypeError(
                "no message type was set".into(),
            )),
        }
    }
}

impl FromNative<ScabbardMessage> for scabbard_v3::ScabbardMessageV3 {
    fn from_native(source: ScabbardMessage) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::ScabbardMessageV3::new();

        match source {
            ScabbardMessage::ConsensusMessage(msg) => proto_msg.set_consensus_message(msg),
        }

        Ok(proto_msg)
    }
}
