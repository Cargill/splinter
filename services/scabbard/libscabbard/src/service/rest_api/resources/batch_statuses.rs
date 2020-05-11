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

use std::time::SystemTime;

use crate::service::state::{BatchInfo, BatchStatus, InvalidTransaction, ValidTransaction};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchInfoResponse<'a> {
    pub id: &'a str,
    pub status: BatchStatusResponse<'a>,
    pub timestamp: SystemTime,
}

impl<'a> From<&'a BatchInfo> for BatchInfoResponse<'a> {
    fn from(info: &'a BatchInfo) -> Self {
        Self {
            id: &info.id,
            status: BatchStatusResponse::from(&info.status),
            timestamp: info.timestamp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "statusType", content = "message")]
pub enum BatchStatusResponse<'a> {
    Unknown,
    Pending,
    Invalid(Vec<InvalidTransactionResponse<'a>>),
    Valid(Vec<ValidTransactionResponse<'a>>),
    Committed(Vec<ValidTransactionResponse<'a>>),
}

impl<'a> From<&'a BatchStatus> for BatchStatusResponse<'a> {
    fn from(status: &'a BatchStatus) -> Self {
        match status {
            BatchStatus::Unknown => BatchStatusResponse::Unknown,
            BatchStatus::Pending => BatchStatusResponse::Pending,
            BatchStatus::Invalid(txns) => BatchStatusResponse::Invalid(
                txns.iter().map(InvalidTransactionResponse::from).collect(),
            ),
            BatchStatus::Valid(txns) => BatchStatusResponse::Valid(
                txns.iter().map(ValidTransactionResponse::from).collect(),
            ),
            BatchStatus::Committed(txns) => BatchStatusResponse::Committed(
                txns.iter().map(ValidTransactionResponse::from).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidTransactionResponse<'a> {
    pub transaction_id: &'a str,
}

impl<'a> From<&'a ValidTransaction> for ValidTransactionResponse<'a> {
    fn from(valid_txn: &'a ValidTransaction) -> Self {
        Self {
            transaction_id: &valid_txn.transaction_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InvalidTransactionResponse<'a> {
    pub transaction_id: &'a str,
    pub error_message: &'a str,
    pub error_data: &'a [u8],
}

impl<'a> From<&'a InvalidTransaction> for InvalidTransactionResponse<'a> {
    fn from(invalid_txn: &'a InvalidTransaction) -> Self {
        Self {
            transaction_id: &invalid_txn.transaction_id,
            error_message: &invalid_txn.error_message,
            error_data: &invalid_txn.error_data,
        }
    }
}
