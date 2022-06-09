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

ALTER TABLE consensus_2pc_action RENAME TO old_consensus_2pc_action;
ALTER TABLE consensus_2pc_event RENAME TO old_consensus_2pc_event;

CREATE TABLE IF NOT EXISTS consensus_2pc_action (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_event (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    event_type                TEXT NOT NULL
    CHECK ( event_type IN ('ALARM', 'DELIVER', 'START', 'VOTE') )
);

INSERT INTO consensus_2pc_action
    (
        id,
        service_id,
        created_at,
        executed_at
    )
    SELECT
        id,
        service_id,
        created_at,
        executed_at
    FROM old_consensus_2pc_action;

INSERT INTO consensus_2pc_event
    (
        id,
        service_id,
        created_at,
        executed_at,
        event_type
    )
    SELECT
        id,
        service_id,
        created_at,
        executed_at,
        event_type
    FROM old_consensus_2pc_event;
