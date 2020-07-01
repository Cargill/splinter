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

use std::fs;
use std::path::Path;

use splinter::transport::multi::MultiTransport;
use splinter::transport::socket::TcpTransport;
use splinter::transport::socket::TlsTransport;
use splinter::transport::tls::{TlsConfig, TlsConfigBuilder};
#[cfg(feature = "ws-transport")]
use splinter::transport::ws::WsTransport;
use splinter::transport::Transport;

use crate::config::Config;
use crate::error::GetTransportError;

type SendableTransport = Box<dyn Transport + Send>;

pub fn build_transport(config: &Config) -> Result<MultiTransport, GetTransportError> {
    let mut transports: Vec<SendableTransport> = vec![];

    // add tcp transport
    // this will be default for endpoints without a prefix
    transports.push(Box::new(TcpTransport::default()));

    // add web socket transport

    // add tls transport
    if !config.no_tls() {
        let tls_config = build_tls_config(&config)?;
        validate_tls_config(&tls_config)?;
        print_tls_config(&tls_config)?;

        transports.push(Box::new(TlsTransport::new(
            tls_config.ca_certs_file().to_owned(),
            tls_config.client_private_key_file().to_string(),
            tls_config.client_cert_file().to_string(),
            tls_config.server_private_key_file().to_string(),
            tls_config.server_cert_file().to_string(),
        )?));

        #[cfg(feature = "ws-transport")]
        transports.push(Box::new(WsTransport::new(Some(&tls_config)).map_err(
            |e| {
                GetTransportError::CertError(format!("Failed to create WebSocket transport: {}", e))
            },
        )?));
    } else {
        #[cfg(feature = "ws-transport")]
        transports.push(Box::new(WsTransport::default()));
    }

    Ok(MultiTransport::new(transports))
}

fn build_tls_config(config: &Config) -> Result<TlsConfig, GetTransportError> {
    let mut builder = TlsConfigBuilder::new()
        .with_client_cert_file(config.tls_client_cert().to_string())
        .with_client_private_key_file(config.tls_client_key().to_string())
        .with_server_cert_file(config.tls_server_cert().to_string())
        .with_server_private_key_file(config.tls_server_key().to_string());

    if config.tls_insecure() {
        warn!("Starting TlsTransport in insecure mode");
    } else {
        builder = builder.with_ca_certs_file(config.tls_ca_file().to_string());
    }

    builder
        .build()
        .map_err(|e| GetTransportError::CertError(format!("TLS config error: {}", e)))
}

fn validate_tls_config(tls_config: &TlsConfig) -> Result<(), GetTransportError> {
    let client_cert = tls_config.client_cert_file();
    if !Path::new(&client_cert).is_file() {
        return Err(GetTransportError::CertError(format!(
            "Must provide a valid client certificate: {}",
            client_cert
        )));
    }

    let server_cert = tls_config.server_cert_file();
    if !Path::new(&server_cert).is_file() {
        return Err(GetTransportError::CertError(format!(
            "Must provide a valid server certificate: {}",
            server_cert
        )));
    }

    let server_key_file = tls_config.server_private_key_file();
    if !Path::new(&server_key_file).is_file() {
        return Err(GetTransportError::CertError(format!(
            "Must provide a valid server key path: {}",
            server_key_file
        )));
    }

    let client_key_file = tls_config.client_private_key_file();
    if !Path::new(&client_key_file).is_file() {
        return Err(GetTransportError::CertError(format!(
            "Must provide a valid client key path: {}",
            client_key_file
        )));
    }

    if let Some(ca_file) = tls_config.ca_certs_file() {
        if !Path::new(&ca_file).is_file() {
            return Err(GetTransportError::CertError(format!(
                "Must provide a valid file containing ca certs: {}",
                ca_file
            )));
        }
    }

    Ok(())
}

fn print_tls_config(tls_config: &TlsConfig) -> Result<(), GetTransportError> {
    debug!(
        "Using client certificate file: {:?}",
        fs::canonicalize(tls_config.client_cert_file())?
    );
    debug!(
        "Using client key file: {:?}",
        fs::canonicalize(tls_config.client_private_key_file())?
    );
    debug!(
        "Using server certificate file: {:?}",
        fs::canonicalize(tls_config.server_cert_file())?
    );
    debug!(
        "Using server key file: {:?}",
        fs::canonicalize(tls_config.server_private_key_file())?
    );
    if let Some(ca_path) = tls_config.ca_certs_file() {
        debug!("Using ca certs file: {:?}", ca_path);
    }

    Ok(())
}
