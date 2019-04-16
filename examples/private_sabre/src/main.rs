// Copyright 2019 Cargill Incorporated
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

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

mod config;
mod error;

use crate::error::ServiceError;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(err) = run() {
        error!("Unable to run Sabre Service: {}", err);
    }
}

fn run() -> Result<(), ServiceError> {
    let matches = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (author: "Contributors to Splinter")
        (about: "Private Sabre Service for Splinter")
        (@arg service_id: -N --("service-id") +takes_value +required
         "the name of this service, as presented to the network")
        (@arg circuit: -C --circuit +takes_value +required
         "the name of the circuit to connect to")
        (@arg verifier: -V --verifier +takes_value +required +multiple
         "the name of a service that will validate transactions")
        (@arg transport: --transport +takes_value
         "transport type for sockets, either raw or tls")
        (@arg ca_file: --("ca-file") +takes_value
         "file path to the trusted ca cert")
        (@arg client_key: --("client-key") +takes_value
         "file path for the TLS key used to connect to a splinterd node")
        (@arg client_cert: --("client-cert") +takes_value
         "file path the cert used to connect to a splinterd node")
        (@arg verbose: -v +multiple "Log verbosely")
    )
    .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::Level::Warn,
        1 => log::Level::Info,
        _ => log::Level::Debug,
    };
    simple_logger::init_with_level(log_level)?;

    let service_config = config::ConfigBuilder::default()
        .with_cli_args(&matches)
        .build()?;

    info!(
        "Started Private Sabre Service ({}/{})",
        service_config.circuit(),
        service_config.service_id()
    );
    info!("Binding on {}", service_config.bind());
    info!(
        "Connecting to network via {} over {}",
        service_config.connect(),
        match service_config.transport_config() {
            &config::TransportConfig::Raw => "raw socket",
            &config::TransportConfig::TLS { .. } => "TLS",
        }
    );
    info!(
        "Transactions verified by [{}]",
        service_config
            .verifiers()
            .iter()
            .map(|v| format!("{}/{}", service_config.circuit(), v))
            .collect::<Vec<_>>()
            .join(", ")
    );

    Ok(())
}
