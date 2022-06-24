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

use crate::store::scabbard_store::alarm::AlarmType;
use crate::store::scabbard_store::diesel::schema::scabbard_alarm;

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_alarm"]
#[primary_key(circuit_id, service_id, alarm_type)]
pub struct ScabbardAlarmModel {
    pub circuit_id: String,
    pub service_id: String,
    pub alarm_type: String,
    pub alarm: i64, // timestamp, when to wake up
}

impl From<&AlarmType> for String {
    fn from(status: &AlarmType) -> Self {
        match *status {
            AlarmType::TwoPhaseCommit => "TWOPHASECOMMIT".into(),
        }
    }
}
