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

use reqwest;

pub fn add_connection(url: &str, addr: &str) {
    let response = reqwest::Client::new()
        .post(&format!("{}/connections/create", url))
        .json(&json!({
            "url": addr.to_string()
        }))
        .send();

    match response {
        Ok(mut r) => {
            println!("{}", r.text().unwrap());
        }
        Err(err) => {
            error!("{}", err);
        }
    };
}

pub fn remove_connection(url: &str, addr: &str) {
    let response = reqwest::Client::new()
        .delete(&format!("{}/connections/delete", url))
        .json(&json!({
            "url": addr.to_string()
        }))
        .send();

    match response {
        Ok(mut r) => {
            println!("{}", r.text().unwrap());
        }
        Err(err) => {
            error!("{}", err);
        }
    };
}

pub fn list_connections(url: &str) {
    let response = reqwest::Client::new()
        .get(&format!("{}/connections/fetch", url))
        .send();

    match response {
        Ok(mut r) => {
            println!("{}", r.text().unwrap());
        }
        Err(err) => {
            error!("{}", err);
        }
    };
}
