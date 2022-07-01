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

ALTER TABLE consensus_2pc_action ADD COLUMN action_type TEXT;

UPDATE consensus_2pc_action as a SET action_type='NOTIFICATION' WHERE EXISTS (
    SELECT * FROM consensus_2pc_notification_action as n WHERE a.id == n.action_id
);

UPDATE consensus_2pc_action as a SET action_type='UPDATE_CONTEXT' WHERE EXISTS (
    SELECT * FROM consensus_2pc_update_context_action as n WHERE a.id == n.action_id
);

UPDATE consensus_2pc_action as a SET action_type='SEND_MESSAGE' WHERE EXISTS (
    SELECT * FROM consensus_2pc_send_message_action as n WHERE a.id == n.action_id
);

ALTER TABLE consensus_2pc_action ADD COLUMN event_id TEXT;

UPDATE consensus_2pc_action SET event_id = (
    SELECT consensus_2pc_event.id as event_id FROM consensus_2pc_event
    INNER JOIN consensus_2pc_action 
    	ON consensus_2pc_action.circuit_id == consensus_2pc_event.circuit_id AND
    	consensus_2pc_action.service_id == consensus_2pc_event.service_id
    ORDER BY consensus_2pc_event.id DESC LIMIT 1
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_action (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    created_at                TEXT DEFAULT (strftime('%Y-%m-%d %H:%M:%f','now', 'localtime')) NOT NULL,
    executed_at               TEXT,
    action_type               TEXT  NOT NULL
     CHECK ( action_type IN ( 
        "UPDATE_CONTEXT", 
        "SEND_MESSAGE", 
        "NOTIFICATION") ),
    event_id                 INTEGER  NOT NULL,
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE,
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

-- Move data from the old tables into the updated tables
INSERT INTO new_consensus_2pc_action
    (
        id,
        circuit_id,
        service_id,
        created_at,
        executed_at,
        action_type,
        event_id
    )
    SELECT
        consensus_2pc_action.id,
        consensus_2pc_action.circuit_id,
        consensus_2pc_action.service_id,
        strftime('%Y-%m-%d %H:%M:%f', consensus_2pc_action.created_at, 'localtime') as created_at,
        strftime('%Y-%m-%d %H:%M:%f', consensus_2pc_action.executed_at, "unixepoch", "localtime") as executed_at,
        action_type,
        event_id
    FROM consensus_2pc_action;

DROP TABLE consensus_2pc_action;
ALTER TABLE new_consensus_2pc_action RENAME TO consensus_2pc_action;

PRAGMA foreign_keys=on;
