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

//! A WebSocket-based transport implementation.
//!
//! The `splinter::transport::ws` module provides a `Transport` implementation
//! on top of an underlying WebSocket.

mod connection;
mod listener;
mod transport;

pub use transport::WsTransport;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::tests;
    use crate::transport::Transport;

    #[test]
    fn test_accepts() {
        let transport = WsTransport::default();
        assert!(transport.accepts("ws://127.0.0.1:18080"));
        assert!(transport.accepts("ws://somewhere.example.com:18080"));
    }

    #[test]
    fn test_transport() {
        let transport = WsTransport::default();

        tests::test_transport(transport, "ws://127.0.0.1:18080");
    }

    #[test]
    fn test_poll() {
        let transport = WsTransport::default();
        tests::test_poll(transport, "ws://127.0.0.1:18081");
    }
}
