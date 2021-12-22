// Copyright 2018-2021 Cargill Incorporated
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

use std::error::Error;

use crate::service::error::ScabbardStateError;

#[derive(Debug)]
pub enum ScabbardError {
    BatchVerificationFailed(Box<dyn Error + Send>),
    ConsensusFailed(ScabbardConsensusManagerError),
    InitializationFailed(Box<dyn Error + Send>),
    Internal(Box<dyn Error + Send>),
    LockPoisoned,
    MessageTypeUnset,
    NotConnected,
    StateInteractionFailed(ScabbardStateError),
}

impl Error for ScabbardError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ScabbardError::BatchVerificationFailed(err) => Some(&**err),
            ScabbardError::ConsensusFailed(err) => Some(err),
            ScabbardError::InitializationFailed(err) => Some(&**err),
            ScabbardError::Internal(err) => Some(&**err),
            ScabbardError::LockPoisoned => None,
            ScabbardError::MessageTypeUnset => None,
            ScabbardError::NotConnected => None,
            ScabbardError::StateInteractionFailed(err) => Some(err),
        }
    }
}

impl std::fmt::Display for ScabbardError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ScabbardError::BatchVerificationFailed(err) => {
                write!(f, "failed to verify batch: {}", err)
            }
            ScabbardError::ConsensusFailed(err) => write!(f, "scabbard consensus failed: {}", err),
            ScabbardError::InitializationFailed(err) => {
                write!(f, "failed to initialize scabbard: {}", err)
            }
            ScabbardError::Internal(err) => {
                write!(f, "internal error occurred: {}", err)
            }
            ScabbardError::LockPoisoned => write!(f, "internal lock poisoned"),
            ScabbardError::MessageTypeUnset => write!(f, "received message with unset type"),
            ScabbardError::NotConnected => {
                write!(f, "attempted to send message, but service isn't connected")
            }
            ScabbardError::StateInteractionFailed(err) => {
                write!(f, "interaction with scabbard state failed: {}", err)
            }
        }
    }
}

impl From<ScabbardConsensusManagerError> for ScabbardError {
    fn from(err: ScabbardConsensusManagerError) -> Self {
        ScabbardError::ConsensusFailed(err)
    }
}

impl From<ScabbardStateError> for ScabbardError {
    fn from(err: ScabbardStateError) -> Self {
        ScabbardError::StateInteractionFailed(err)
    }
}

#[derive(Debug)]
pub struct ScabbardConsensusManagerError(pub Box<dyn Error + Send>);

impl Error for ScabbardConsensusManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.0)
    }
}

impl std::fmt::Display for ScabbardConsensusManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "scabbard consensus manager failed: {}", self.0)
    }
}
