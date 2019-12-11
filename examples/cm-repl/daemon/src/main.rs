// Copyright 2018 Cargill Incorporated
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

extern crate clap;
#[macro_use]
extern crate log;

mod rest_api;

use clap::{clap_app, crate_version};
use ctrlc;
use flexi_logger::{style, DeferredNow, LogSpecBuilder, Logger};
use log::Record;
use rest_api::start_rest_api;
use splinter::{
    mesh::Mesh,
    network::connection_manager::ConnectionManager,
    transport::{raw::RawTransport, Incoming, Transport},
};

use std::thread;

fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    let level = record.level();
    write!(
        w,
        "[{}] T[{:?}] {} [{}] {}",
        now.now().format("%Y-%m-%d %H:%M:%S%.3f"),
        thread::current().name().unwrap_or("<unnamed>"),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        style(level, &record.args()),
    )
}

fn main() {
    let app = clap_app!(cm_poc =>
        (version: crate_version!())
        (about: "Proof of concept demostrating connection manager usage")
        (@arg bind: +takes_value "Rest api address")
        (@arg endpoint: +takes_value "Node endpoint")
        (@arg verbose: -v --verbose +multiple "Increase output verbosity"));

    let matches = app.get_matches();

    let bind = matches.value_of("bind").unwrap_or("0.0.0.0:3030");
    let endpoint = matches.value_of("endpoint").unwrap_or("tcp://0.0.0.0:3040");

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let mut log_spec_builder = LogSpecBuilder::new();
    log_spec_builder.default(log_level);
    log_spec_builder.module("hyper", log::LevelFilter::Warn);
    log_spec_builder.module("tokio", log::LevelFilter::Warn);

    Logger::with(log_spec_builder.build())
        .format(log_format)
        .start()
        .expect("Failed to create logger");

    let mesh = Mesh::new(512, 512);

    let mut transport = RawTransport::default();
    let mut listener = transport
        .listen(endpoint)
        .expect("Failed to create listener");

    let mut cm = ConnectionManager::new(
        mesh.get_life_cycle(),
        mesh.get_sender(),
        Box::new(transport),
    );

    let mesh_clone = mesh.clone();
    let _ = thread::spawn(move || {
        for connection_result in listener.incoming() {
            info!("Recieved connection");
            let connection = match connection_result {
                Ok(c) => c,
                Err(err) => return error!("{:?}", err),
            };

            mesh_clone.add(connection).unwrap();
        }
    });

    let mesh_incoming = mesh.incoming();
    let _ = thread::spawn(move || loop {
        match mesh_incoming.recv() {
            Ok(msg) => {
                info!("Received Message: {:?}", msg);
            }
            Err(err) => {
                error!("{:?}", err);
            }
        }
    });

    let connector = cm.start().expect("Failed to create connection manager");

    let shutdown_handle = cm.shutdown_handle().unwrap();
    let (rest_api_shutdown_handle, join_handle) = start_rest_api(connector, &bind);

    ctrlc::set_handler(move || {
        info!("shutting down");
        shutdown_handle.shutdown();
        if let Err(err) = rest_api_shutdown_handle.shutdown() {
            error!("Failed to shutdown rest api gracefully: {:?}", err);
        }
    })
    .expect("Error setting up ctrl-c handler");

    cm.await_shutdown();
    join_handle.join().unwrap();
}
