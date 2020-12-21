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

//! An identity provider that extracts the public key from a Cylinder JWT

use std::sync::{Arc, Mutex};

use cylinder::{jwt::JsonWebTokenParser, Verifier};

use crate::error::InternalError;
use crate::rest_api::auth::{AuthorizationHeader, BearerToken};

use super::IdentityProvider;

/// Extracts the public key from a Cylinder JWT
///
/// This provider only accepts `AuthorizationHeader::Bearer(BearerToken::Cylinder(token))`
/// authorizations, and the inner token must be a valid Cylinder JWT.
#[derive(Clone)]
pub struct CylinderKeyIdentityProvider {
    /// The verifier is wrapped in an `Arc<Mutex<_>>` to ensure this struct is `Sync`
    verifier: Arc<Mutex<Box<dyn Verifier>>>,
}

impl CylinderKeyIdentityProvider {
    /// Creates a new Cylinder key identity provider
    pub fn new(verifier: Arc<Mutex<Box<dyn Verifier>>>) -> Self {
        Self { verifier }
    }
}

impl IdentityProvider for CylinderKeyIdentityProvider {
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<String>, InternalError> {
        let token = match authorization {
            AuthorizationHeader::Bearer(BearerToken::Cylinder(token)) => token,
            _ => return Ok(None),
        };

        Ok(
            JsonWebTokenParser::new(&**self.verifier.lock().map_err(|_| {
                InternalError::with_message(
                    "Cylinder key identity provider's verifier lock poisoned".into(),
                )
            })?)
            .parse(token)
            .ok()
            .map(|parsed_token| parsed_token.issuer().as_hex()),
        )
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
