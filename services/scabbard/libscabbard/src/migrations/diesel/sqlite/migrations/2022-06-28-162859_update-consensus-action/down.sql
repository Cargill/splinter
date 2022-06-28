-- Copyright 2018-2022 Cargill Incorporated
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the License.
-- You may obtain a copy of the License at
--
--     http://www.apache.org/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the License.
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS new_consensus_2pc_action (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id)
);

-- Move data from the old tables into the updated tables
INSERT INTO new_consensus_2pc_action
    (
        id,
        circuit_id,
        service_id,
        created_at,
        executed_at
    )
    SELECT
        consensus_2pc_action.id,
        consensus_2pc_action.circuit_id,
        consensus_2pc_action.service_id,
        strftime('%Y-%m-%d %H:%M:%S', consensus_2pc_action.created_at) as created_at,
        strftime('%s', consensus_2pc_action.executed_at) as executed_at
    FROM consensus_2pc_action;

DROP TABLE consensus_2pc_action;
ALTER TABLE new_consensus_2pc_action RENAME TO consensus_2pc_action;
