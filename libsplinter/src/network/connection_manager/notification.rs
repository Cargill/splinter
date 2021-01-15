// Copyright 2018-2021 Cargill Incorporated
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

use super::error::ConnectionManagerError;

/// Messages that will be dispatched to all subscription handlers
#[derive(Debug, PartialEq, Clone)]
pub enum ConnectionManagerNotification {
    Connected {
        endpoint: String,
        connection_id: String,
        identity: String,
    },
    FatalConnectionError {
        endpoint: String,
        error: ConnectionManagerError,
    },
    InboundConnection {
        endpoint: String,
        connection_id: String,
        identity: String,
    },
    Disconnected {
        endpoint: String,
        identity: String,
    },
    NonFatalConnectionError {
        endpoint: String,
        attempts: u64,
        identity: String,
    },
}
