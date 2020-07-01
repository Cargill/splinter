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

pub struct TlsConfig {
    ca_certs_file: Option<String>,
    server_cert_file: String,
    server_private_key_file: String,
    client_cert_file: String,
    client_private_key_file: String,
}

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

#[derive(Default)]
pub struct TlsConfigBuilder {
    ca_certs_file: Option<String>,
    server_cert_file: Option<String>,
    server_private_key_file: Option<String>,
    client_cert_file: Option<String>,
    client_private_key_file: Option<String>,
}

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

#[derive(Debug)]
pub enum TlsConfigBuilderError {
    MissingField(String),
}

impl std::error::Error for TlsConfigBuilderError {}

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

#[cfg(test)]
pub(super) mod tests {
    use openssl::asn1::Asn1Time;
    use openssl::bn::{BigNum, MsbOption};
    use openssl::hash::MessageDigest;
    use openssl::pkey::{PKey, PKeyRef, Private};
    use openssl::rsa::Rsa;
    use openssl::x509::extension::{BasicConstraints, ExtendedKeyUsage, KeyUsage};
    use openssl::x509::{X509NameBuilder, X509Ref, X509};

    // Make a certificate and private key for the Certificate Authority
    pub fn make_ca_cert() -> (PKey<Private>, X509) {
        let rsa = Rsa::generate(2048).unwrap();
        let privkey = PKey::from_rsa(rsa).unwrap();

        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name.append_entry_by_text("CN", "ca test").unwrap();
        let x509_name = x509_name.build();

        let mut cert_builder = X509::builder().unwrap();
        cert_builder.set_version(2).unwrap();
        cert_builder.set_subject_name(&x509_name).unwrap();
        cert_builder.set_issuer_name(&x509_name).unwrap();
        cert_builder.set_pubkey(&privkey).unwrap();

        let not_before = Asn1Time::days_from_now(0).unwrap();
        cert_builder.set_not_before(&not_before).unwrap();
        let not_after = Asn1Time::days_from_now(365).unwrap();
        cert_builder.set_not_after(&not_after).unwrap();

        cert_builder
            .append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        cert_builder
            .append_extension(KeyUsage::new().key_cert_sign().build().unwrap())
            .unwrap();

        cert_builder
            .sign(&privkey, MessageDigest::sha256())
            .unwrap();
        let cert = cert_builder.build();

        (privkey, cert)
    }

    // Make a certificate and private key signed by the given CA cert and private key
    pub fn make_ca_signed_cert(
        ca_cert: &X509Ref,
        ca_privkey: &PKeyRef<Private>,
    ) -> (PKey<Private>, X509) {
        let rsa = Rsa::generate(2048).unwrap();
        let privkey = PKey::from_rsa(rsa).unwrap();

        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name.append_entry_by_text("CN", "localhost").unwrap();
        let x509_name = x509_name.build();

        let mut cert_builder = X509::builder().unwrap();
        cert_builder.set_version(2).unwrap();
        let serial_number = {
            let mut serial = BigNum::new().unwrap();
            serial.rand(159, MsbOption::MAYBE_ZERO, false).unwrap();
            serial.to_asn1_integer().unwrap()
        };
        cert_builder.set_serial_number(&serial_number).unwrap();
        cert_builder.set_subject_name(&x509_name).unwrap();
        cert_builder
            .set_issuer_name(ca_cert.subject_name())
            .unwrap();
        cert_builder.set_pubkey(&privkey).unwrap();
        let not_before = Asn1Time::days_from_now(0).unwrap();
        cert_builder.set_not_before(&not_before).unwrap();
        let not_after = Asn1Time::days_from_now(365).unwrap();
        cert_builder.set_not_after(&not_after).unwrap();

        cert_builder
            .append_extension(
                ExtendedKeyUsage::new()
                    .server_auth()
                    .client_auth()
                    .build()
                    .unwrap(),
            )
            .unwrap();

        cert_builder
            .sign(&ca_privkey, MessageDigest::sha256())
            .unwrap();
        let cert = cert_builder.build();

        (privkey, cert)
    }
}
