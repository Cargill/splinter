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

use actix_web::{client::Client, http::StatusCode, web, Error, HttpResponse};
use serde::{Deserialize, Serialize};
use splinter::protocol;

use crate::rest_api::GameroomdData;

use super::{ErrorResponse, SuccessResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponseData {
    email: String,
    public_key: String,
    encrypted_private_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthData {
    pub email: String,
    pub hashed_password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UsernamePassword {
    pub username: String,
    pub hashed_password: String,
}

impl From<AuthData> for UsernamePassword {
    fn from(auth_data: AuthData) -> Self {
        Self {
            username: auth_data.email,
            hashed_password: auth_data.hashed_password,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Token {
    pub message: String,
    pub user_id: String,
    pub token: String,
}

#[derive(Clone, Deserialize)]
struct Keys {
    pub data: Vec<Key>,
}

#[derive(Debug, Clone, Deserialize)]
struct Key {
    pub public_key: String,
    pub encrypted_private_key: String,
    pub user_id: String,
    pub display_name: String,
}

#[derive(Serialize)]
struct NewKey {
    pub public_key: String,
    pub encrypted_private_key: String,
    pub display_name: String,
}

pub async fn login(
    auth_data: web::Json<AuthData>,
    client: web::Data<Client>,
    gameroomd_data: web::Data<GameroomdData>,
) -> Result<HttpResponse, Error> {
    // forward login to splinterd
    let mut login_response = client
        .post(format!("{}/biome/login", &gameroomd_data.splinterd_url))
        .header(
            "SplinterProtocolVersion",
            protocol::BIOME_PROTOCOL_VERSION.to_string(),
        )
        .send_json(&UsernamePassword::from(auth_data.into_inner()))
        .await?;

    let token: Token = match login_response.status() {
        StatusCode::OK => login_response.json().await?,
        StatusCode::BAD_REQUEST => {
            let body = login_response.body().await?;
            let body_value: serde_json::Value = serde_json::from_slice(&body)?;
            let message = match body_value.get("message") {
                Some(value) => value.as_str().unwrap_or("Request was malformed."),
                None => "Request malformed.",
            };
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&message)));
        }
        StatusCode::UNAUTHORIZED => {
            let body = login_response.body().await?;
            let body_value: serde_json::Value = serde_json::from_slice(&body)?;
            let message = match body_value.get("Unauthorized") {
                Some(value) => value.as_str().unwrap_or("Unauthorized user"),
                None => "Unauthorized user",
            };
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::unauthorized(&message)));
        }
        _ => {
            error!(
                "Internal Server Error. Splinterd responded with error {}",
                login_response.status(),
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };
    let authorization = format!("Bearer {}", token.token);

    // Get user's key
    let mut key_response = client
        .get(format!("{}/biome/keys", &gameroomd_data.splinterd_url))
        .header(
            "SplinterProtocolVersion",
            protocol::BIOME_PROTOCOL_VERSION.to_string(),
        )
        .header("Authorization", authorization)
        .send()
        .await?;

    let key = match key_response.status() {
        StatusCode::OK => {
            let data = key_response.json::<Keys>().await?.data;
            if let Some(key) = data.get(0) {
                key.clone()
            } else {
                error!("User does not have any keys");
                return Ok(
                    HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                );
            }
        }
        StatusCode::BAD_REQUEST => {
            let body = login_response.body().await?;
            let body_value: serde_json::Value = serde_json::from_slice(&body)?;
            let message = match body_value.get("message") {
                Some(value) => value.as_str().unwrap_or("Request was malformed."),
                None => "Request malformed.",
            };
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&message)));
        }
        _ => {
            error!(
                "Internal Server Error. Splinterd responded with error {}",
                login_response.status(),
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };

    Ok(
        HttpResponse::Ok().json(SuccessResponse::new(AuthResponseData {
            email: key.display_name,
            public_key: key.public_key,
            encrypted_private_key: key.encrypted_private_key,
        })),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreate {
    pub email: String,
    pub hashed_password: String,
    pub encrypted_private_key: String,
    pub public_key: String,
}

pub async fn register(
    json_wrapper: web::Json<UserCreate>,
    client: web::Data<Client>,
    gameroomd_data: web::Data<GameroomdData>,
) -> Result<HttpResponse, Error> {
    let new_user = json_wrapper.into_inner();

    let username_password = UsernamePassword {
        username: new_user.email.clone(),
        hashed_password: new_user.hashed_password.clone(),
    };

    let mut registered_response = client
        .post(format!("{}/biome/register", &gameroomd_data.splinterd_url))
        .header(
            "SplinterProtocolVersion",
            protocol::BIOME_PROTOCOL_VERSION.to_string(),
        )
        .send_json(&username_password)
        .await?;

    match registered_response.status() {
        StatusCode::OK => (),
        StatusCode::BAD_REQUEST => {
            let body = registered_response.body().await?;
            let body_value: serde_json::Value = serde_json::from_slice(&body)?;
            let message = match body_value.get("message") {
                Some(value) => value.as_str().unwrap_or("Request was malformed."),
                None => "Request malformed.",
            };
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&message)));
        }
        _ => {
            error!(
                "Internal Server Error. Splinterd responded with error {}",
                registered_response.status(),
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };

    let mut login_response = client
        .post(format!("{}/biome/login", &gameroomd_data.splinterd_url))
        .header(
            "SplinterProtocolVersion",
            protocol::BIOME_PROTOCOL_VERSION.to_string(),
        )
        .send_json(&username_password)
        .await?;

    let token: Token = match login_response.status() {
        StatusCode::OK => login_response.json().await?,
        StatusCode::BAD_REQUEST => {
            let body = login_response.body().await?;
            let body_value: serde_json::Value = serde_json::from_slice(&body)?;
            let message = match body_value.get("message") {
                Some(value) => value.as_str().unwrap_or("Request was malformed."),
                None => "Request malformed.",
            };
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&message)));
        }
        _ => {
            error!(
                "Internal Server Error. Splinterd responded with error {}",
                login_response.status(),
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };
    let authorization = format!("Bearer {}", token.token);

    // Create Key
    let create_key_response = client
        .post(format!("{}/biome/keys", &gameroomd_data.splinterd_url))
        .header(
            "SplinterProtocolVersion",
            protocol::BIOME_PROTOCOL_VERSION.to_string(),
        )
        .header("Authorization", authorization)
        .send_json(&NewKey {
            display_name: new_user.email.clone(),
            encrypted_private_key: new_user.encrypted_private_key.clone(),
            public_key: new_user.public_key.clone(),
        })
        .await?;

    match create_key_response.status() {
        StatusCode::OK => (),
        _ => {
            error!(
                "Internal Server Error. Failed to create key {}",
                create_key_response.status(),
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    }

    Ok(HttpResponse::Ok().json(AuthResponseData {
        email: new_user.email,
        public_key: new_user.public_key,
        encrypted_private_key: new_user.encrypted_private_key,
    }))
}
