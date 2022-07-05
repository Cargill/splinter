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

-- Add the executed_epoch column

-- Default all executed columns to the most recent epoch
CREATE TABLE temp_consensus_2pc_event (
    id                        INTEGER PRIMARY KEY,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    event_type                TEXT NOT NULL,
    created_at                TEXT NOT NULL,
    executed_at               TEXT
);

INSERT INTO temp_consensus_2pc_event
SELECT id, circuit_id, service_id, event_type,
       strftime('%Y-%m-%d %H:%M:%f', created_at) as created_at,
       strftime('%Y-%m-%d %H:%M:%f', executed_at, "unixepoch") as executed_at
FROM consensus_2pc_event;

DROP TABLE consensus_2pc_event;

CREATE TABLE consensus_2pc_event (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    event_type                TEXT NOT NULL
        CHECK ( event_type IN ('ALARM', 'DELIVER', 'START', 'VOTE') ),
    created_at                TEXT
        DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')) NOT NULL,
    executed_at               TEXT,
    executed_epoch            BIGINT,
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

INSERT INTO consensus_2pc_event
SELECT t.id, t.circuit_id, t.service_id, t.event_type, t.created_at, t.executed_at,
       CASE
           WHEN t.executed_at IS NOT NULL THEN ctx.epoch
           ELSE NULL
       END as executed_epoch
FROM temp_consensus_2pc_event t,
     consensus_2pc_context ctx
WHERE ctx.circuit_id = t.circuit_id
  AND ctx.service_id = t.service_id;

DROP TABLE temp_consensus_2pc_event;

PRAGMA foreign_keys=on;
