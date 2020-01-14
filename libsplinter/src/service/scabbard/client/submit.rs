// Copyright 2020 Cargill Incorporated
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

use std::{fmt, str};

use protobuf::Message;
use reqwest::{blocking::Client, Url};

use sawtooth_sdk::messages::batch::BatchList;

use super::Error;

const BATCHES_ENDPOINT: &str = "/batches";
const HTTP_SCHEME: &str = "http";

pub fn submit_batch_list(url: &str, batch_list: &BatchList) -> Result<String, Error> {
    let url = format!("{}{}", url, BATCHES_ENDPOINT);
    let url = Url::parse(&url).map_err(|err| Error(format!("invalid URL: {}", err)))?;
    if url.scheme() != HTTP_SCHEME {
        return Err(Error(format!(
            "unsupported scheme ({}) in URL: {}",
            url.scheme(),
            url
        )));
    }

    let body = batch_list.write_to_bytes()?;

    let request = Client::new().post(url).body(body);
    let response = request
        .send()
        .map_err(|err| Error(format!("request failed: {}", err)))?
        .error_for_status()
        .map_err(|err| Error(format!("received error status code: {}", err)))?;

    let batch_link: Link = response
        .json()
        .map_err(|err| Error(format!("failed to parse response as batch link: {}", err)))?;

    Ok(batch_link.link)
}

// pub fn wait_for_batch(url: &str, wait: u64) -> Result<StatusResponse, Error> {
//     let url_with_wait_query = format!("{}&wait={}", url, wait);
//
//     // Validate url
//
//     let hyper_uri = match url_with_wait_query.parse::<Uri>() {
//         Ok(uri) => uri,
//         Err(e) => return Err(Error(format!("invalid URL: {}: {}", e, url))),
//     };
//
//     match hyper_uri.scheme() {
//         Some(scheme) => {
//             if scheme != "http" {
//                 return Err(Error(format!(
//                     "unsupported scheme ({}) in URL: {}",
//                     scheme, url
//                 )));
//             }
//         }
//         None => {
//             return Err(Error(format!("no scheme in URL: {}", url)));
//         }
//     }
//
//     let mut core = tokio_core::reactor::Core::new()?;
//     let handle = core.handle();
//     let client = Client::configure().build(&handle);
//
//     let work = client.get(hyper_uri).and_then(|res| {
//         if res.status() == StatusCode::ServiceUnavailable {
//             panic!("Service Unavailable");
//         } else {
//             res.body().concat2().and_then(move |chunks| {
//                 future::ok(serde_json::from_slice::<StatusResponse>(&chunks).unwrap())
//             })
//         }
//     });
//
//     let body = core.run(work)?;
//
//     Ok(body)
// }

#[derive(Deserialize, Debug)]
struct Link {
    link: String,
}

#[derive(Deserialize, Debug)]
pub struct BatchStatus {
    id: String,
    status: String,
    invalid_transactions: Vec<InvalidTransaction>,
}

#[derive(Deserialize, Debug)]
pub struct InvalidTransaction {
    id: String,
    message: String,
}

#[derive(Deserialize, Debug)]
pub struct StatusResponse {
    data: Vec<BatchStatus>,
    link: String,
}

// impl StatusResponse {
//     pub fn is_finished(&self) -> bool {
//         self.data.iter().all(|x| x.status == "COMMITTED")
//             || self.data.iter().any(|x| x.status == "INVALID")
//     }
// }

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{\"link\": {}}}", self.link)
    }
}

impl fmt::Display for BatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut invalid_txn_string_vec = Vec::new();
        for txn in &self.invalid_transactions {
            invalid_txn_string_vec.push(txn.to_string());
        }
        write!(
            f,
            "{{\"id\": \"{}\", \"status\": \"{}\", \"invalid_transactions\": [{}]}}",
            self.id,
            self.status,
            invalid_txn_string_vec.join(",")
        )
    }
}

impl fmt::Display for InvalidTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{\"id\": \"{}\", \"message\": \"{}\"}}",
            self.id, self.message
        )
    }
}

impl fmt::Display for StatusResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut data_string_vec = Vec::new();
        for data in &self.data {
            data_string_vec.push(data.to_string());
        }

        write!(
            f,
            "StatusResponse {{\"data\":[{}], \"link\": \"{}\"}}",
            data_string_vec.join(","),
            self.link
        )
    }
}
