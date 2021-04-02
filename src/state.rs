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

//! Contains functions which assist with fetching state

use futures::Stream;
use futures::{future, Future};
use hyper::client::{Client, Request};
use hyper::Method;
use std::str;

use crate::error::CliError;

pub fn get_state_with_prefix(url: &str, prefix: &str) -> Result<Vec<StateEntry>, CliError> {
    let post_url = String::from(url) + "/state?address=" + prefix;
    let hyper_uri = match post_url.parse::<hyper::Uri>() {
        Ok(uri) => uri,
        Err(e) => return Err(CliError::UserError(format!("Invalid URL: {}: {}", e, url))),
    };

    match hyper_uri.scheme() {
        Some(scheme) => {
            if scheme != "http" {
                return Err(CliError::UserError(format!(
                    "Unsupported scheme ({}) in URL: {}",
                    scheme, url
                )));
            }
        }
        None => {
            return Err(CliError::UserError(format!("No scheme in URL: {}", url)));
        }
    }

    let mut core = tokio_core::reactor::Core::new()?;
    let handle = core.handle();
    let client = Client::configure().build(&handle);

    let req = Request::new(Method::Get, hyper_uri);

    let work = client.request(req).and_then(|res| {
        res.body().concat2().and_then(move |chunks| {
            future::ok(serde_json::from_slice::<JsonStateEntry>(&chunks).unwrap())
        })
    });

    let response = core.run(work)?;

    Ok(response.data)
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonStateEntry {
    data: Vec<StateEntry>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct StateEntry {
    pub address: String,
    pub data: String,
}
