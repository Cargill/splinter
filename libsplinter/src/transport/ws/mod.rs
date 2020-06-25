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
    use crate::transport::tls::tests::{make_ca_cert, make_ca_signed_cert};
    use crate::transport::tls::{TlsConfig, TlsConfigBuilder};
    use crate::transport::Transport;

    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempdir::TempDir;

    fn write_file(mut temp_dir: PathBuf, file_name: &str, bytes: &[u8]) -> String {
        temp_dir.push(file_name);
        let path = temp_dir.to_str().unwrap().to_string();
        let mut file = File::create(path.to_string()).unwrap();
        file.write_all(bytes).unwrap();

        path
    }

    pub fn create_test_tls_config(temp_dir: &TempDir, insecure: bool) -> TlsConfig {
        let mut builder = TlsConfigBuilder::new();

        // Generate Certificate Authority keys and certificate
        let (ca_key, ca_cert) = make_ca_cert();

        // create temp directory to store ca.cert
        let temp_dir_path = temp_dir.path();

        if !insecure {
            let ca_path_file = write_file(
                temp_dir_path.to_path_buf(),
                "ca.cert",
                &ca_cert.to_pem().unwrap(),
            );
            builder = builder.with_ca_certs_file(ca_path_file);
        }

        // Generate client and server keys and certificates
        let (client_key, client_cert) = make_ca_signed_cert(&ca_cert, &ca_key);
        let (server_key, server_cert) = make_ca_signed_cert(&ca_cert, &ca_key);

        let client_cert_file = write_file(
            temp_dir_path.to_path_buf(),
            "client.cert",
            &client_cert.to_pem().unwrap(),
        );

        let client_key_file = write_file(
            temp_dir_path.to_path_buf(),
            "client.key",
            &client_key.private_key_to_pem_pkcs8().unwrap(),
        );

        let server_cert_file = write_file(
            temp_dir_path.to_path_buf(),
            "server.cert",
            &server_cert.to_pem().unwrap(),
        );

        let server_key_file = write_file(
            temp_dir_path.to_path_buf(),
            "server.key",
            &server_key.private_key_to_pem_pkcs8().unwrap(),
        );

        if !insecure {}

        builder
            .with_server_cert_file(server_cert_file)
            .with_server_private_key_file(server_key_file)
            .with_client_cert_file(client_cert_file)
            .with_client_private_key_file(client_key_file)
            .build()
            .unwrap()
    }

    #[test]
    fn test_ws_accepts() {
        let transport = WsTransport::default();
        assert!(transport.accepts("ws://127.0.0.1:18080"));
        assert!(transport.accepts("ws://somewhere.example.com:18080"));
    }

    #[test]
    fn test_ws_transport() {
        let transport = WsTransport::default();

        tests::test_transport(transport, "ws://127.0.0.1:18080");
    }

    #[test]
    fn test_ws_poll() {
        let transport = WsTransport::default();
        tests::test_poll(transport, "ws://127.0.0.1:18081");
    }

    #[test]
    fn test_wss_accepts() {
        let transport = WsTransport::default();
        assert!(transport.accepts("wss://127.0.0.1:18080"));
        assert!(transport.accepts("wss://somewhere.example.com:18080"));
    }

    #[test]
    fn test_wss_transport() {
        let temp_dir = TempDir::new("test-wss-transport").unwrap();
        let config = create_test_tls_config(&temp_dir, true);
        let transport = WsTransport::new(Some(&config)).unwrap();
        tests::test_transport(transport, "wss://127.0.0.1:18082");
    }

    #[test]
    fn test_wss_poll() {
        let temp_dir = TempDir::new("test-wss-poll").unwrap();
        let config = create_test_tls_config(&temp_dir, false);
        let transport = WsTransport::new(Some(&config)).unwrap();
        tests::test_poll(transport, "wss://127.0.0.1:18083");
    }
}
