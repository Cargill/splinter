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

use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use openssl::error::ErrorStack;
use openssl::ssl::{
    Error as OpensslError, HandshakeError, SslAcceptor, SslConnector, SslFiletype, SslMethod,
    SslStream, SslVerifyMode,
};
use url::{ParseError, Url};

use std::error::Error;
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;

use crate::transport::{
    AcceptError, ConnectError, Connection, DisconnectError, ListenError, Listener, RecvError,
    SendError, Transport,
};

use super::frame::{Frame, FrameError, FrameNegotiation, FrameRef, FrameVersion};

/// tls:// is deprecated, tcps:// should be used instead
const DEPRECATED_PROTOCOL_PREFIX: &str = "tls://";
const PROTOCOL_PREFIX: &str = "tcps://";

pub struct TlsTransport {
    connector: SslConnector,
    acceptor: SslAcceptor,
}

impl TlsTransport {
    pub fn new(
        ca_cert: Option<String>,
        client_key: String,
        client_cert: String,
        server_key: String,
        server_cert: String,
    ) -> Result<Self, TlsInitError> {
        let client_cert_path = Path::new(&client_cert);
        let client_key_path = Path::new(&client_key);
        let server_cert_path = Path::new(&server_cert);
        let server_key_path = Path::new(&server_key);

        // Build TLS Connector
        let mut connector = SslConnector::builder(SslMethod::tls())?;
        connector.set_private_key_file(&client_key_path, SslFiletype::PEM)?;
        connector.set_certificate_chain_file(client_cert_path)?;
        connector.check_private_key()?;

        // Build TLS Acceptor
        let mut acceptor = SslAcceptor::mozilla_modern(SslMethod::tls())?;
        acceptor.set_private_key_file(server_key_path, SslFiletype::PEM)?;
        acceptor.set_certificate_chain_file(&server_cert_path)?;
        acceptor.check_private_key()?;

        // if ca_cert is provided set as accept cert, otherwise set verify to none
        let (acceptor, connector) = {
            if let Some(ca_cert) = ca_cert {
                let ca_cert_path = Path::new(&ca_cert);
                acceptor.set_ca_file(ca_cert_path)?;
                connector.set_ca_file(ca_cert_path)?;
                connector.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
                acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
                let connector = connector.build();
                let acceptor = acceptor.build();
                (acceptor, connector)
            } else {
                connector.set_verify(SslVerifyMode::NONE);
                acceptor.set_verify(SslVerifyMode::NONE);
                let connector = connector.build();
                let acceptor = acceptor.build();
                (acceptor, connector)
            }
        };

        Ok(TlsTransport {
            connector,
            acceptor,
        })
    }
}

fn endpoint_to_dns_name(endpoint: &str) -> Result<String, ParseError> {
    let mut address = String::from("tcp://");
    address.push_str(endpoint);
    let url = Url::parse(&address)?;
    let dns_name = match url.domain() {
        Some(d) if d.parse::<Ipv4Addr>().is_ok() => "localhost",
        Some(d) if d.parse::<Ipv6Addr>().is_ok() => "localhost",
        Some(d) => d,
        None => "localhost",
    };
    Ok(String::from(dns_name))
}

impl Transport for TlsTransport {
    fn accepts(&self, address: &str) -> bool {
        address.starts_with(PROTOCOL_PREFIX)
            || address.starts_with(DEPRECATED_PROTOCOL_PREFIX)
            || !address.contains("://")
    }

    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError> {
        if !self.accepts(endpoint) {
            return Err(ConnectError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                endpoint
            )));
        }

        let address = if let Some(address) = endpoint.strip_prefix(PROTOCOL_PREFIX) {
            address
        } else if let Some(address) = endpoint.strip_prefix(DEPRECATED_PROTOCOL_PREFIX) {
            address
        } else {
            endpoint
        };

        let dns_name = endpoint_to_dns_name(address)?;

        let stream = TcpStream::connect(address)?;
        let mut tls_stream = self.connector.connect(&dns_name, stream)?;

        let frame_version = FrameNegotiation::outbound(FrameVersion::V1, FrameVersion::V1)
            .negotiate(&mut tls_stream)
            .map_err(|err| match err {
                FrameError::UnsupportedVersion => ConnectError::ProtocolError(
                    "Unable to connect; remote version is not with in range".into(),
                ),
                FrameError::IoError(err) => ConnectError::from(err),
                e => ConnectError::ProtocolError(format!("Unexpected protocol error: {}", e)),
            })?;

        tls_stream.get_ref().set_nonblocking(true)?;
        let connection = TlsConnection {
            frame_version,
            stream: tls_stream,
        };
        Ok(Box::new(connection))
    }

    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError> {
        if !self.accepts(bind) {
            return Err(ListenError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                bind
            )));
        }

        let address = if let Some(address) = bind.strip_prefix(PROTOCOL_PREFIX) {
            address
        } else if let Some(address) = bind.strip_prefix(DEPRECATED_PROTOCOL_PREFIX) {
            address
        } else {
            bind
        };

        Ok(Box::new(TlsListener {
            listener: TcpListener::bind(address).map_err(|err| {
                ListenError::IoError(format!("Failed to bind to {}", address), err)
            })?,
            acceptor: self.acceptor.clone(),
        }))
    }
}

pub struct TlsListener {
    listener: TcpListener,
    acceptor: SslAcceptor,
}

impl Listener for TlsListener {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError> {
        let (stream, _) = self.listener.accept()?;
        let mut tls_stream = self.acceptor.accept(stream)?;

        let frame_version = FrameNegotiation::inbound(FrameVersion::V1)
            .negotiate(&mut tls_stream)
            .map_err(|err| match err {
                FrameError::UnsupportedVersion => AcceptError::ProtocolError(format!(
                    "Local {} protocol version {} not supported by remote",
                    PROTOCOL_PREFIX,
                    FrameVersion::V1
                )),
                FrameError::IoError(err) => AcceptError::from(err),
                err => AcceptError::ProtocolError(format!("Unexpected protocol error: {}", err)),
            })?;

        tls_stream.get_ref().set_nonblocking(true)?;
        let connection = TlsConnection {
            frame_version,
            stream: tls_stream,
        };
        Ok(Box::new(connection))
    }

    fn endpoint(&self) -> String {
        format!("tcps://{}", self.listener.local_addr().unwrap())
    }
}

pub struct TlsConnection {
    frame_version: FrameVersion,
    stream: SslStream<TcpStream>,
}

impl Connection for TlsConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        match FrameRef::new(self.frame_version, message).write(&mut self.stream) {
            Err(FrameError::IoError(e)) => Err(SendError::from(e)),
            Err(err) => Err(SendError::ProtocolError(err.to_string())),
            Ok(_) => Ok(()),
        }
    }

    fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
        match Frame::read(&mut self.stream) {
            Err(FrameError::IoError(e)) => Err(RecvError::from(e)),
            Err(err) => Err(RecvError::ProtocolError(err.to_string())),
            Ok(frame) => Ok(frame.into_inner()),
        }
    }

    fn remote_endpoint(&self) -> String {
        format!("tcps://{}", self.stream.get_ref().peer_addr().unwrap())
    }

    fn local_endpoint(&self) -> String {
        format!("tcps://{}", self.stream.get_ref().local_addr().unwrap())
    }

    fn disconnect(&mut self) -> Result<(), DisconnectError> {
        // returns Shutdown state
        self.stream.shutdown()?;
        Ok(())
    }

    fn evented(&self) -> &dyn Evented {
        self
    }
}

impl TlsConnection {
    #[deprecated(
        since = "0.3.13",
        note = "connections should only be made through the TlsTransport, as it negotiates the \
        wire protocol version"
    )]
    pub fn new(stream: SslStream<TcpStream>) -> Self {
        TlsConnection {
            frame_version: FrameVersion::V1,
            stream,
        }
    }
}

impl AsRawFd for TlsConnection {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.get_ref().as_raw_fd()
    }
}

impl Evented for TlsConnection {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

#[derive(Debug)]
pub enum TlsInitError {
    ProtocolError(String),
}

impl Error for TlsInitError {}

impl fmt::Display for TlsInitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TlsInitError::ProtocolError(msg) => write!(f, "unable to initialize TLS: {}", msg),
        }
    }
}

impl From<ErrorStack> for TlsInitError {
    fn from(error: ErrorStack) -> Self {
        TlsInitError::ProtocolError(format!("Openssl Error: {}", error))
    }
}

impl From<HandshakeError<TcpStream>> for AcceptError {
    fn from(handshake_error: HandshakeError<TcpStream>) -> Self {
        AcceptError::ProtocolError(format!("TLS Handshake Err: {}", handshake_error))
    }
}

impl From<HandshakeError<TcpStream>> for ConnectError {
    fn from(handshake_error: HandshakeError<TcpStream>) -> Self {
        ConnectError::ProtocolError(format!("TLS Handshake Err: {}", handshake_error))
    }
}

impl From<ParseError> for ConnectError {
    fn from(parse_error: ParseError) -> Self {
        ConnectError::ParseError(format!("Parse Error: {:?}", parse_error.to_string()))
    }
}

impl From<OpensslError> for DisconnectError {
    fn from(openssl_error: OpensslError) -> Self {
        DisconnectError::ProtocolError(format!("Openssl Err: {}", openssl_error))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::transport::tests;
    use crate::transport::tls::tests::{make_ca_cert, make_ca_signed_cert};

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

    pub fn create_test_tls_transport(insecure: bool) -> TlsTransport {
        // Genearte Certificat Authority keys and certificate
        let (ca_key, ca_cert) = make_ca_cert();

        // create temp directory to store ca.cert
        let temp_dir = TempDir::new("tls-transport-test").unwrap();
        let temp_dir_path = temp_dir.path();
        let ca_path_file = {
            if insecure {
                None
            } else {
                let ca_path_file = write_file(
                    temp_dir_path.to_path_buf(),
                    "ca.cert",
                    &ca_cert.to_pem().unwrap(),
                );
                Some(ca_path_file)
            }
        };

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

        // Create TLsTransport
        TlsTransport::new(
            ca_path_file,
            client_key_file,
            client_cert_file,
            server_key_file,
            server_cert_file,
        )
        .unwrap()
    }

    #[test]
    fn test_transport() {
        let transport = create_test_tls_transport(true);
        tests::test_transport(transport, "127.0.0.1:0");
    }

    #[test]
    fn test_transport_explicit_protocol() {
        let transport = create_test_tls_transport(true);
        tests::test_transport(transport, "tcps://127.0.0.1:0");
    }

    #[test]
    fn test_transport_deprecated_explicit_protocol() {
        let transport = create_test_tls_transport(true);
        tests::test_transport(transport, "tls://127.0.0.1:0");
    }

    #[cfg(not(unix))]
    #[test]
    fn test_poll() {
        let transport = create_test_tls_transport(true);
        tests::test_poll(transport, "127.0.0.1:0");
    }

    #[test]
    fn test_transport_no_verify() {
        let transport = create_test_tls_transport(false);
        tests::test_transport(transport, "127.0.0.1:0");
    }

    #[cfg(not(unix))]
    #[test]
    fn test_poll_no_verify() {
        let transport = create_test_tls_transport(false);
        tests::test_poll(
            transport,
            "127.0.0.1:0",
            Ready::readable() | Ready::writable(),
        );
    }
}
