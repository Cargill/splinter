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

//! Contains functions which assist with batch submission to a REST API

use serde_json;
use hyper;
use hyper::StatusCode;
use hyper::Method;
use hyper::client::{Client, Request};
use std::{str, fmt};
use hyper::header::{ContentLength, ContentType};
use futures::{future, Future};
use futures::Stream;
use tokio_core;

use sawtooth_sdk::messages::batch::BatchList;

use error::CliError;
use protobuf::Message;

pub fn submit_batch_list(url: &str, batch_list: &BatchList) -> Result<String, CliError> {
    let post_url = String::from(url) + "/batches";
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

    let bytes = batch_list.write_to_bytes()?;

    let mut req = Request::new(Method::Post, hyper_uri);
    req.headers_mut().set(ContentType::octet_stream());
    req.headers_mut().set(ContentLength(bytes.len() as u64));
    req.set_body(bytes);

    let work = client.request(req).and_then(|res| {
        res.body().concat2().and_then(move |chunks| {
            future::ok(serde_json::from_slice::<Link>(&chunks).unwrap())
        })
    });

    let batch_link = core.run(work)?;
    println!("Response Body:\n{:?}", batch_link);

    Ok(batch_link.link)
}

pub fn wait_for_batch(url: &str, wait: u64) -> Result<bool, CliError> {
    let url_with_wait_query  = format!("{}&wait={}", url, wait);

    // Validate url

    let hyper_uri = match url_with_wait_query.parse::<hyper::Uri>() {
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

    let work = client.get(hyper_uri).and_then(|res| {
        if res.status() == StatusCode::ServiceUnavailable {
            panic!("Service Unavailable");
        } else {
            res.body().concat2().and_then(move |chunks| {
                future::ok(serde_json::from_slice::<StatusResponse>(&chunks).unwrap())
            })
        }
    });

    let body = core.run(work)?;
    println!("Response Body:\n{}", body);

    Ok(body.data.iter().all(|x| x.status == "COMMITTED") ||
        body.data.iter().any(|x| x.status == "INVALID"))
}

#[derive(Deserialize, Debug)]
struct Link {
    link: String
}

#[derive(Deserialize, Debug)]
struct Data {
    id: String,
    status: String,
    invalid_transactions: Vec<InvalidTransaction>
}

#[derive(Deserialize, Debug)]
struct InvalidTransaction {
    id: String,
    message: String
}

#[derive(Deserialize, Debug)]
struct StatusResponse {
    data: Vec<Data>,
    link: String
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{\"link\": {}}}", self.link)
    }
}

impl fmt::Display for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut invalid_txn_string_vec = Vec::new();
        for txn in &self.invalid_transactions{
           invalid_txn_string_vec.push(txn.to_string());
        }
        write!(f, "{{\"id\": \"{}\", \"status\": \"{}\", \"invalid_transactions\": [{}]}}",
            self.id, self.status, invalid_txn_string_vec.join(","))
    }
}

impl fmt::Display for InvalidTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{\"id\": \"{}\", \"message\": \"{}\"}}", self.id, self.message )
    }
}

impl fmt::Display for StatusResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut data_string_vec = Vec::new();
        for data in &self.data {
            data_string_vec.push(data.to_string());
        }

        write!(f, "StatusResponse {{\"data\":[{}], \"link\": \"{}\"}}",
            data_string_vec.join(","), self.link )
    }
}
