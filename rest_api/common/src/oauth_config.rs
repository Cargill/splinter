// Copyright 2018-2022 Cargill Incorporated
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

use splinter::oauth::store::InflightOAuthRequestStore;

/// OAuth configurations that are supported out-of-the-box by the Splinter REST API.
pub enum OAuthConfig {
    Azure {
        /// The client ID of the Azure OAuth app
        client_id: String,
        /// The client secret of the Azure OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the Azure OAuth app
        redirect_url: String,
        /// The URL of the OpenID discovery document for the Azure OAuth app
        oauth_openid_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    /// OAuth provided by GitHub
    GitHub {
        /// The client ID of the GitHub OAuth app
        client_id: String,
        /// The client secret of the GitHub OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the GitHub OAuth app
        redirect_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    Google {
        /// The client ID of the Google OAuth app
        client_id: String,
        /// The client secret of the Google OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the Google OAuth app
        redirect_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    OpenId {
        /// The client ID of the OpenId OAuth app
        client_id: String,
        /// The client secret of the OpenId OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the OpenId OAuth app
        redirect_url: String,
        /// The URL of the OpenID discovery document for the OpenId OAuth app
        oauth_openid_url: String,
        /// Additional parameters to add to auth requests made to the OpenID OAuth provider
        auth_params: Option<Vec<(String, String)>>,
        /// Additional scopes to request from the OpenID OAuth provider
        scopes: Option<Vec<String>>,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
}
