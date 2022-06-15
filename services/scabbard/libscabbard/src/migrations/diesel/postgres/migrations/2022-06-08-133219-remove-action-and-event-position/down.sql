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

-- Drop foreign key constraints
ALTER TABLE consensus_2pc_update_context_action DROP CONSTRAINT consensus_2pc_update_context_action_action_id_fkey;
ALTER TABLE consensus_2pc_update_context_action_participant DROP CONSTRAINT consensus_2pc_update_context_action_participant_action_id_fkey;
ALTER TABLE consensus_2pc_send_message_action DROP CONSTRAINT consensus_2pc_send_message_action_action_id_fkey;
ALTER TABLE consensus_2pc_notification_action DROP CONSTRAINT consensus_2pc_notification_action_action_id_fkey;

ALTER TABLE consensus_2pc_deliver_event DROP CONSTRAINT consensus_2pc_deliver_event_event_id_fkey;
ALTER TABLE consensus_2pc_start_event DROP CONSTRAINT consensus_2pc_start_event_event_id_fkey;
ALTER TABLE consensus_2pc_vote_event DROP CONSTRAINT consensus_2pc_vote_event_event_id_fkey;

-- Recreate the tables with the position column
CREATE TABLE IF NOT EXISTS new_consensus_2pc_action (
    id                        BIGSERIAL PRIMARY KEY,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    position                  INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_event (
    id                        BIGSERIAL PRIMARY KEY,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    position                  INTEGER NOT NULL,
    event_type                TEXT NOT NULL
    CHECK ( event_type IN ('ALARM', 'DELIVER', 'START', 'VOTE') )
);

-- Move data from the old tables into the updated tables setting position to the value stored in id
INSERT INTO new_consensus_2pc_action
    (
        id,
        service_id,
        created_at,
        executed_at,
        position
    )
    SELECT
        id,
        service_id,
        created_at,
        executed_at,
        id
    FROM consensus_2pc_action;

INSERT INTO new_consensus_2pc_event
    (
        id,
        service_id,
        created_at,
        executed_at,
        event_type,
        position
    )
    SELECT
        id,
        service_id,
        created_at,
        executed_at,
        event_type,
        id
    FROM consensus_2pc_event;

-- Drop the old tables
DROP TABLE consensus_2pc_action;
DROP TABLE consensus_2pc_event;

-- Rename the new tables to the old names
ALTER TABLE new_consensus_2pc_action RENAME TO consensus_2pc_action;
ALTER TABLE new_consensus_2pc_event RENAME TO consensus_2pc_event;

-- Recreate the foreign key constraints
ALTER TABLE consensus_2pc_update_context_action ADD CONSTRAINT consensus_2pc_update_context_action_action_id_fkey FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id);
ALTER TABLE consensus_2pc_update_context_action_participant ADD CONSTRAINT consensus_2pc_update_context_action_participant_action_id_fkey FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id);
ALTER TABLE consensus_2pc_send_message_action ADD CONSTRAINT consensus_2pc_send_message_action_action_id_fkey FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id);
ALTER TABLE consensus_2pc_notification_action ADD CONSTRAINT consensus_2pc_notification_action_action_id_fkey FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id);

ALTER TABLE consensus_2pc_deliver_event ADD CONSTRAINT consensus_2pc_deliver_event_event_id_fkey FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id);
ALTER TABLE consensus_2pc_start_event ADD CONSTRAINT consensus_2pc_start_event_event_id_fkey FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id);
ALTER TABLE consensus_2pc_vote_event ADD CONSTRAINT consensus_2pc_vote_event_event_id_fkey FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id);
