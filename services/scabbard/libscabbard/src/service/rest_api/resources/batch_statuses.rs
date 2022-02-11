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

use std::time::SystemTime;

use serde::{ser::SerializeSeq, Serialize, Serializer};

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
    #[serde(serialize_with = "empty_array")]
    Unknown,
    #[serde(serialize_with = "empty_array")]
    Pending,
    Invalid(Vec<InvalidTransactionResponse<'a>>),
    Valid(Vec<ValidTransactionResponse<'a>>),
    Committed(Vec<ValidTransactionResponse<'a>>),
}

fn empty_array<S>(serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_seq(None)?.end()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{self, Value};

    fn struct_to_value(object: &impl Serialize) -> Value {
        serde_json::from_str(&serde_json::to_string(object).expect("error serializing"))
            .expect("error deserializing")
    }

    fn assert_json(actual: &impl Serialize, expected: &str) {
        assert_eq!(
            struct_to_value(actual),
            serde_json::from_str::<Value>(expected).expect("error deserializing")
        );
    }

    #[test]
    fn batch_status_response_serializes_correctly() {
        assert_json(
            &BatchStatusResponse::Unknown,
            r#"{
              "statusType": "Unknown",
              "message": []
            }"#,
        );

        assert_json(
            &BatchStatusResponse::Pending,
            r#"{
              "statusType": "Pending",
              "message": []
            }"#,
        );

        assert_json(
            &BatchStatusResponse::Invalid(vec![InvalidTransactionResponse {
                transaction_id: "txid",
                error_message: "message",
                error_data: &[0, 1, 2],
            }]),
            r#"{
              "statusType": "Invalid",
              "message": [
                {
                  "transaction_id": "txid",
                  "error_message": "message",
                  "error_data": [
                    0,
                    1,
                    2
                  ]
                }
              ]
            }"#,
        );

        assert_json(
            &BatchStatusResponse::Valid(vec![ValidTransactionResponse {
                transaction_id: "txid",
            }]),
            r#"{
              "statusType": "Valid",
              "message": [
                {
                  "transaction_id": "txid"
                }
              ]
            }"#,
        );

        assert_json(
            &BatchStatusResponse::Committed(vec![ValidTransactionResponse {
                transaction_id: "txid",
            }]),
            r#"{
              "statusType": "Committed",
              "message": [
                {
                  "transaction_id": "txid"
                }
              ]
            }"#,
        );
    }
}
