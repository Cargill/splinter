/*
 * Copyright 2018-2020 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */

use actix_web::Result;
use futures::{Future, Stream};
use hyper::{Client as HyperClient, StatusCode, Uri};
use tokio::runtime::Runtime;

use crate::error::{ConfigurationError, GetNodeError};

#[derive(Debug)]
pub struct GameroomConfig {
    rest_api_endpoint: String,
    database_url: String,
    splinterd_url: String,
}

impl GameroomConfig {
    pub fn rest_api_endpoint(&self) -> &str {
        &self.rest_api_endpoint
    }
    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn splinterd_url(&self) -> &str {
        &self.splinterd_url
    }
}

pub struct GameroomConfigBuilder {
    rest_api_endpoint: Option<String>,
    database_url: Option<String>,
    splinterd_url: Option<String>,
}

impl Default for GameroomConfigBuilder {
    fn default() -> Self {
        Self {
            rest_api_endpoint: Some("127.0.0.1:8000".to_owned()),
            database_url: Some(
                "postgres://gameroom:gameroom_example@postgres:5432/gameroom".to_owned(),
            ),
            splinterd_url: Some("http://127.0.0.1:8080".to_owned()),
        }
    }
}

impl GameroomConfigBuilder {
    pub fn with_cli_args(&mut self, matches: &clap::ArgMatches<'_>) -> Self {
        Self {
            rest_api_endpoint: matches
                .value_of("bind")
                .map(ToOwned::to_owned)
                .or_else(|| self.rest_api_endpoint.take()),

            database_url: matches
                .value_of("database_url")
                .map(ToOwned::to_owned)
                .or_else(|| self.database_url.take()),

            splinterd_url: matches
                .value_of("splinterd_url")
                .map(ToOwned::to_owned)
                .or_else(|| self.splinterd_url.take()),
        }
    }

    pub fn build(mut self) -> Result<GameroomConfig, ConfigurationError> {
        Ok(GameroomConfig {
            rest_api_endpoint: self
                .rest_api_endpoint
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("rest_api_endpoint".to_owned()))?,
            database_url: self
                .database_url
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("database_url".to_owned()))?,
            splinterd_url: self
                .splinterd_url
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("splinterd_url".to_owned()))?,
        })
    }
}

pub fn get_node(splinterd_url: &str) -> Result<NodeInfo, GetNodeError> {
    let mut runtime = Runtime::new()
        .map_err(|err| GetNodeError(format!("Failed to get set up runtime: {}", err)))?;
    let client = HyperClient::new();
    let splinterd_url = splinterd_url.to_owned();
    let uri = format!("{}/status", splinterd_url)
        .parse::<Uri>()
        .map_err(|err| GetNodeError(format!("Failed to get set up request: {}", err)))?;

    runtime.block_on(
        client
            .get(uri)
            .map_err(|err| GetNodeError(format!("Failed to get splinter node metadata: {}", err)))
            .and_then(|resp| {
                if resp.status() != StatusCode::OK {
                    return Err(GetNodeError(format!(
                        "Failed to get splinter node metadata. Splinterd responded with status {}",
                        resp.status()
                    )));
                }
                let body = resp
                    .into_body()
                    .concat2()
                    .wait()
                    .map_err(|err| {
                        GetNodeError(format!("Failed to get splinter node metadata: {}", err))
                    })?
                    .to_vec();

                serde_json::from_slice(&body).map_err(|err| {
                    GetNodeError(format!("Failed to get splinter node metadata: {}", err))
                })
            }),
    )
}

#[derive(Clone, Deserialize)]
pub struct NodeInfo {
    #[serde(alias = "node_id")]
    pub identity: String,
    #[serde(alias = "network_endpoints")]
    pub endpoints: Vec<String>,
}
