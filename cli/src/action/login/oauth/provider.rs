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

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use reqwest::blocking::Client;

use super::callback;
use super::{error::OAuth2ProviderError, UserTokens};

/// An OAuth2 Provider.
///
/// This struct interacts with an OAuth2 provider following the OAuth2 specification.  Providers
/// that uses custom flows are not necessarily supported.
pub struct Provider {
    provider_id: String,
    client_id: String,
    client_secret: String,
    auth_url: String,
    token_url: String,
    scopes: Vec<String>,
}

impl Provider {
    pub fn new(
        provider_id: String,
        client_id: String,
        client_secret: String,
        auth_url: String,
        token_url: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            provider_id,
            client_id,
            client_secret,
            auth_url,
            token_url,
            scopes,
        }
    }

    pub fn get_tokens(&self) -> Result<UserTokens, OAuth2ProviderError> {
        let callback =
            callback::OAuth2Callback::new().map_err(|err| OAuth2ProviderError(err.to_string()))?;

        let auth_url = AuthUrl::new(self.auth_url.clone())
            .map_err(|err| OAuth2ProviderError(err.to_string()))?;
        let token_url = TokenUrl::new(self.token_url.clone())
            .map_err(|err| OAuth2ProviderError(err.to_string()))?;
        let redirect_url = RedirectUrl::new(callback.callback_url())
            .map_err(|err| OAuth2ProviderError(err.to_string()))?;

        let client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
            auth_url,
            Some(token_url),
        )
        .set_redirect_url(redirect_url);

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut authorize_request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);
        for scope in self.scopes.iter() {
            authorize_request = authorize_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = authorize_request.url();

        info!("Opening {}...", auth_url);
        webbrowser::open(&auth_url.to_string()).map_err(|err| {
            debug!("Unable to open authorization URL: {}", err);
            OAuth2ProviderError(
                "Unable to open Authorization URL in browser; \
                ensure that a default browser is installed."
                    .into(),
            )
        })?;

        let (auth_code, csrf_state) = callback
            .recv()
            .expect("Unable to receive authorization code");

        if csrf_token.secret() != &csrf_state {
            return Err(OAuth2ProviderError(format!(
                "{} did not return the same CSRF token",
                self.provider_id
            )));
        }

        let token_result = client
            .exchange_code(AuthorizationCode::new(auth_code))
            .set_pkce_verifier(pkce_verifier)
            .request(|req| {
                Client::new()
                    .execute(to_reqwest_req(req)?)
                    .map(to_oauth2_res)
                    .map_err(|_err| {
                        OAuth2ProviderError("Unable to execute for OAuth2 Access Token".into())
                    })
            })
            .map_err(|_err| OAuth2ProviderError("Unable to request OAuth2 Access Token".into()))?;

        Ok(UserTokens {
            provider_type: self.provider_id.clone(),
            access_token: token_result.access_token().secret().clone(),
            refresh_token: None,
        })
    }
}

fn to_reqwest_req(
    req: oauth2::HttpRequest,
) -> Result<reqwest::blocking::Request, OAuth2ProviderError> {
    use oauth2::http::method::Method as OAuth2Method;
    use reqwest::Method as ReqwestMethod;
    use reqwest::Url as ReqwestUrl;
    let mut reqwest_req = reqwest::blocking::Request::new(
        match req.method {
            OAuth2Method::GET => ReqwestMethod::GET,
            OAuth2Method::POST => ReqwestMethod::POST,
            OAuth2Method::PUT => ReqwestMethod::PUT,
            OAuth2Method::DELETE => ReqwestMethod::DELETE,
            OAuth2Method::HEAD => ReqwestMethod::HEAD,
            OAuth2Method::OPTIONS => ReqwestMethod::OPTIONS,
            OAuth2Method::CONNECT => ReqwestMethod::CONNECT,
            OAuth2Method::PATCH => ReqwestMethod::PATCH,
            OAuth2Method::TRACE => ReqwestMethod::TRACE,
            _ => unreachable!(),
        },
        ReqwestUrl::parse(req.url.as_str())
            .map_err(|_| OAuth2ProviderError("The library did not provide a valid URL".into()))?,
    );

    let body = reqwest_req.body_mut();
    *body = Some(reqwest::blocking::Body::from(req.body));

    let header_map = reqwest_req.headers_mut();
    for (name, val) in req.headers.into_iter() {
        if let Some(name) = name {
            header_map.insert(
                name.as_str()
                    .parse::<reqwest::header::HeaderName>()
                    .map_err(|_| {
                        OAuth2ProviderError(
                            "The library did not provide a valid header name".into(),
                        )
                    })?,
                reqwest::header::HeaderValue::from_bytes(val.as_bytes()).map_err(|_| {
                    OAuth2ProviderError("The library did not provide a valid header value".into())
                })?,
            );
        }
    }

    Ok(reqwest_req)
}

fn to_oauth2_res(res: reqwest::blocking::Response) -> oauth2::HttpResponse {
    use oauth2::http::StatusCode;
    oauth2::HttpResponse {
        status_code: StatusCode::from_u16(res.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        body: res
            .bytes()
            .map(|bs| bs.as_ref().to_vec())
            .unwrap_or_else(|_| Vec::new()),
        headers: oauth2::http::header::HeaderMap::new(),
    }
}
