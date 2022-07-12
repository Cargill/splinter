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

//! Contains struct and trait implementation for `SignedTimestamp` artifacts

use std::time::SystemTime;

use sawtooth::artifact::{Artifact, ArtifactCreator, ArtifactCreatorFactory};
use sawtooth::error::InternalError;
use serde::{Deserialize, Serialize};
use splinter::public_key::PublicKey;

use super::ScabbardPublishingContext;

/// The header for a SignedTimestamp
///
/// This is the componenet that will be turned to bytes and signed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTimestampHeader {
    timestamp: SystemTime,
    public_key: Vec<u8>,
}

impl SignedTimestampHeader {
    /// Create a new `SignedTimestampHeader`
    pub fn new(timestamp: SystemTime, public_key: PublicKey) -> Result<Self, InternalError> {
        Ok(SignedTimestampHeader {
            timestamp,
            public_key: public_key.into_bytes(),
        })
    }
}

/// The artifact for Scabbard V3
#[derive(Debug, Clone)]
pub struct SignedTimestamp {
    header: SignedTimestampHeader,
    signature: String,
}

impl SignedTimestamp {
    pub fn new(header: SignedTimestampHeader, signature: String) -> Self {
        SignedTimestamp { header, signature }
    }

    pub fn header(&self) -> &SignedTimestampHeader {
        &self.header
    }
}

impl Artifact for SignedTimestamp {
    type Identifier = String;

    fn artifact_id(&self) -> &Self::Identifier {
        &self.signature
    }
}

/// Artifact creator for `SignedTimestamp`
#[derive(Debug, Default, Clone)]
pub struct SignedTimestampCreator {}

impl SignedTimestampCreator {
    pub fn new() -> Self {
        SignedTimestampCreator {}
    }
}

impl ArtifactCreator for SignedTimestampCreator {
    type Context = ScabbardPublishingContext;
    type Input = SystemTime;
    type Artifact = SignedTimestamp;

    /// Create a new `SignedTimestamp`
    ///
    /// Takes a `SystemTime` as input. The input and the public key from the
    /// `ScabbardPublishingContext` are used to create a `SignedTimestampHeader`. The header is
    /// then turned to bytes and signed. Using the header and the resulting signature to created a
    /// `SignedTimestamp`.
    fn create(
        &self,
        context: &mut Self::Context,
        input: Self::Input,
    ) -> Result<Self::Artifact, InternalError> {
        let header = SignedTimestampHeader::new(
            input,
            PublicKey::from(
                context
                    .signer()
                    .public_key()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            ),
        )?;

        // get timestamp bytes
        let header_bytes =
            serde_json::to_vec(&header).map_err(|err| InternalError::from_source(Box::new(err)))?;

        let signature = context
            .signer()
            .sign(&header_bytes)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .as_hex();

        Ok(SignedTimestamp { header, signature })
    }
}

/// Factory for creating an instance of `SignedTimestampCreator`
pub struct SignedTimestampCreatorFactory {}

impl ArtifactCreatorFactory for SignedTimestampCreatorFactory {
    type ArtifactCreator = SignedTimestampCreator;

    /// Create a new `SignedTimestampCreator`
    fn new_creator(&self) -> Result<SignedTimestampCreator, InternalError> {
        Ok(SignedTimestampCreator::new())
    }
}
