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

PRAGMA foreign_keys=off;

CREATE TABLE IF NOT EXISTS new_consensus_2pc_context_participant (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    process                   TEXT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') OR vote IS NULL ),
    PRIMARY KEY (circuit_id, service_id, process),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id)
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_update_context_action_participant (
    action_id                 INTEGER PRIMARY KEY,
    process                   TEXT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') OR vote IS NULL ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE,
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_update_context_action(action_id) ON DELETE CASCADE
);

INSERT INTO new_consensus_2pc_context_participant
    (
        circuit_id,
        service_id,
        epoch,
        process,
        vote
    )
    SELECT
        circuit_id,
        service_id,
        epoch,
        process,
        vote
    FROM consensus_2pc_context_participant;

INSERT INTO new_consensus_2pc_update_context_action_participant
    (
        action_id,
        process,
        vote
    )
    SELECT
        action_id,
        process,
        vote
    FROM consensus_2pc_update_context_action_participant;

DROP TABLE consensus_2pc_context_participant;
DROP TABLE consensus_2pc_update_context_action_participant;

ALTER TABLE new_consensus_2pc_context_participant 
  RENAME TO consensus_2pc_context_participant;
ALTER TABLE new_consensus_2pc_update_context_action_participant 
  RENAME TO consensus_2pc_update_context_action_participant;

PRAGMA foreign_keys=on;
