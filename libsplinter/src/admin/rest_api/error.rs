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

use std::error::Error;

#[derive(Debug)]
pub enum ProposalFetchError {
    NotFound(String),
    InternalError(String),
}

impl Error for ProposalFetchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProposalFetchError::NotFound(_) => None,
            ProposalFetchError::InternalError(_) => None,
        }
    }
}

impl std::fmt::Display for ProposalFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ProposalFetchError::NotFound(msg) => write!(f, "Proposal not found: {}", msg),
            ProposalFetchError::InternalError(msg) => write!(f, "Ran into internal error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub enum ProposalListError {
    InternalError(String),
}

impl Error for ProposalListError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProposalListError::InternalError(_) => None,
        }
    }
}

impl std::fmt::Display for ProposalListError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ProposalListError::InternalError(msg) => write!(f, "Ran into internal error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub enum CircuitFetchError {
    NotFound(String),
    CircuitStoreError(String),
}

impl Error for CircuitFetchError {}

impl std::fmt::Display for CircuitFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CircuitFetchError::NotFound(msg) => write!(f, "Circuit not found: {}", msg),
            CircuitFetchError::CircuitStoreError(msg) => write!(f, "{}", msg),
        }
    }
}

#[derive(Debug)]
pub enum CircuitListError {
    CircuitStoreError(String),
}

impl Error for CircuitListError {}

impl std::fmt::Display for CircuitListError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CircuitListError::CircuitStoreError(msg) => write!(f, "{}", msg),
        }
    }
}
