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

#[cfg(feature = "ws-transport")]
use std::path::Path;

#[cfg(feature = "ws-transport")]
use openssl::error::ErrorStack;
#[cfg(feature = "ws-transport")]
use openssl::ssl::{SslAcceptor, SslConnector, SslFiletype, SslMethod, SslVerifyMode};

#[cfg(feature = "ws-transport")]
pub struct TlsConfig {
    ca_certs_file: Option<String>,
    server_cert_file: String,
    server_private_key_file: String,
    client_cert_file: String,
    client_private_key_file: String,
}

#[cfg(feature = "ws-transport")]
impl TlsConfig {
    pub fn ca_certs_file(&self) -> &Option<String> {
        &self.ca_certs_file
    }

    pub fn server_cert_file(&self) -> &str {
        &self.server_cert_file
    }

    pub fn server_private_key_file(&self) -> &str {
        &self.server_private_key_file
    }

    pub fn client_cert_file(&self) -> &str {
        &self.client_cert_file
    }

    pub fn client_private_key_file(&self) -> &str {
        &self.client_private_key_file
    }
}

#[cfg(feature = "ws-transport")]
#[derive(Default)]
pub struct TlsConfigBuilder {
    ca_certs_file: Option<String>,
    server_cert_file: Option<String>,
    server_private_key_file: Option<String>,
    client_cert_file: Option<String>,
    client_private_key_file: Option<String>,
}

#[cfg(feature = "ws-transport")]
impl TlsConfigBuilder {
    pub fn new() -> Self {
        TlsConfigBuilder {
            ca_certs_file: None,
            server_cert_file: None,
            server_private_key_file: None,
            client_cert_file: None,
            client_private_key_file: None,
        }
    }

    pub fn with_ca_certs_file(mut self, ca_certs_file: String) -> Self {
        self.ca_certs_file = Some(ca_certs_file);
        self
    }

    pub fn with_server_cert_file(mut self, server_cert_file: String) -> Self {
        self.server_cert_file = Some(server_cert_file);
        self
    }

    pub fn with_server_private_key_file(mut self, server_private_key_file: String) -> Self {
        self.server_private_key_file = Some(server_private_key_file);
        self
    }

    pub fn with_client_cert_file(mut self, client_cert_file: String) -> Self {
        self.client_cert_file = Some(client_cert_file);
        self
    }

    pub fn with_client_private_key_file(mut self, client_private_key_file: String) -> Self {
        self.client_private_key_file = Some(client_private_key_file);
        self
    }

    pub fn build(self) -> Result<TlsConfig, TlsConfigBuilderError> {
        let ca_certs_file = self.ca_certs_file;
        let server_cert_file = self
            .server_cert_file
            .ok_or_else(|| TlsConfigBuilderError::MissingField("server_cert_file".to_string()))?;
        let server_private_key_file = self.server_private_key_file.ok_or_else(|| {
            TlsConfigBuilderError::MissingField("server_private_key_file".to_string())
        })?;
        let client_cert_file = self
            .client_cert_file
            .ok_or_else(|| TlsConfigBuilderError::MissingField("client_cert_file".to_string()))?;
        let client_private_key_file = self.client_private_key_file.ok_or_else(|| {
            TlsConfigBuilderError::MissingField("client_private_key_file".to_string())
        })?;

        Ok(TlsConfig {
            ca_certs_file,
            server_cert_file,
            server_private_key_file,
            client_cert_file,
            client_private_key_file,
        })
    }
}

#[cfg(feature = "ws-transport")]
#[derive(Debug)]
pub enum TlsConfigBuilderError {
    MissingField(String),
}

#[cfg(feature = "ws-transport")]
impl std::error::Error for TlsConfigBuilderError {}

#[cfg(feature = "ws-transport")]
impl std::fmt::Display for TlsConfigBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            TlsConfigBuilderError::MissingField(ref s) => {
                write!(f, "Missing required field '{}' in TLS configuration", s)
            }
        }
    }
}

#[cfg(feature = "ws-transport")]
pub(super) fn build_connector(config: &TlsConfig) -> Result<SslConnector, ErrorStack> {
    let mut builder = SslConnector::builder(SslMethod::tls())?;

    builder.set_private_key_file(
        Path::new(config.client_private_key_file()),
        SslFiletype::PEM,
    )?;
    builder.set_certificate_chain_file(Path::new(config.client_cert_file()))?;
    builder.check_private_key()?;

    if let Some(ca_certs_file) = config.ca_certs_file() {
        builder.set_ca_file(Path::new(ca_certs_file))?;
    } else {
        builder.set_verify(SslVerifyMode::NONE);
    }

    Ok(builder.build())
}

#[cfg(feature = "ws-transport")]
pub(super) fn build_acceptor(config: &TlsConfig) -> Result<SslAcceptor, ErrorStack> {
    let mut builder = SslAcceptor::mozilla_modern(SslMethod::tls())?;

    builder.set_private_key_file(
        Path::new(config.server_private_key_file()),
        SslFiletype::PEM,
    )?;
    builder.set_certificate_chain_file(Path::new(config.server_cert_file()))?;
    builder.check_private_key()?;

    if let Some(ca_certs_file) = config.ca_certs_file() {
        builder.set_ca_file(Path::new(ca_certs_file))?;
    } else {
        builder.set_verify(SslVerifyMode::NONE);
    }

    Ok(builder.build())
}
