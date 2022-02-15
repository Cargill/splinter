// Copyright 2018-2022 Cargill Incorporated
//

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

use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum RESTError {
    BadRequest(String),
    NotFound(String),
    InternalError(String, Option<Box<dyn Error>>),
    NotAuthorized,
}

impl std::error::Error for RESTError {}

impl Display for RESTError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RESTError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            RESTError::NotFound(url) => write!(f, "Could not find resource for: {}", url),
            RESTError::InternalError(msg, Some(err)) => {
                write!(f, "Internal Error: {}: {}", msg, err)
            }
            RESTError::InternalError(msg, None) => write!(f, "Internal Error: {}", msg),
            RESTError::NotAuthorized => write!(f, "Not Authorized"),
        }
    }
}

#[cfg(feature = "rest-api-actix-web-4")]
impl actix_web_4::ResponseError for RESTError {}

impl RESTError {
    pub fn bad_request<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::BadRequest(msg.into())
    }

    pub fn not_found(url: String) -> Self {
        Self::NotFound(url)
    }

    pub fn internal_error<S>(msg: S, err: Option<Box<dyn Error>>) -> Self
    where
        S: Into<String>,
    {
        Self::InternalError(msg.into(), err)
    }
}
