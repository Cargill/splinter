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

-- Rename the existing tables
ALTER TABLE consensus_2pc_update_context_action RENAME TO old_consensus_2pc_update_context_action;
ALTER TABLE consensus_2pc_send_message_action RENAME TO old_consensus_2pc_send_message_action;
ALTER TABLE consensus_2pc_notification_action RENAME TO old_consensus_2pc_notification_action;
ALTER TABLE consensus_2pc_update_context_action_participant RENAME TO old_consensus_2pc_update_context_action_participant;
ALTER TABLE consensus_2pc_deliver_event RENAME TO old_consensus_2pc_deliver_event;
ALTER TABLE consensus_2pc_start_event RENAME TO old_consensus_2pc_start_event;
ALTER TABLE consensus_2pc_vote_event RENAME TO old_consensus_2pc_vote_event;

-- Recreate the tables without the service_id columns or foreign key constraints
CREATE TABLE IF NOT EXISTS consensus_2pc_update_context_action (
    action_id                 INTEGER PRIMARY KEY,
    coordinator               TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    last_commit_epoch         BIGINT,
    state                     TEXT NOT NULL
    CHECK ( state IN ( 'WAITINGFORSTART', 'VOTING', 'WAITINGFORVOTE', 'ABORT', 'COMMIT', 'WAITINGFORVOTEREQUEST', 'VOTED') ),
    vote_timeout_start        BIGINT
    CHECK ( (vote_timeout_start IS NOT NULL) OR ( state != 'VOTING') ),
    vote                      TEXT
    CHECK ( (vote IN ('TRUE' , 'FALSE')) OR ( state != 'VOTED') ),
    decision_timeout_start    BIGINT
    CHECK ( (decision_timeout_start IS NOT NULL) OR ( state != 'VOTED') ),
    action_alarm  BIGINT,
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_send_message_action (
    action_id                 INTEGER PRIMARY KEY,
    epoch                     BIGINT NOT NULL,
    receiver_service_id       TEXT NOT NULL,
    message_type              TEXT NOT NULL
    CHECK ( message_type IN ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT') ),
    vote_response             TEXT
    CHECK ( (vote_response IN ('TRUE', 'FALSE')) OR (message_type != 'VOTERESPONSE') ),
    vote_request              BINARY
    CHECK ( (vote_request IS NOT NULL) OR (message_type != 'VOTEREQUEST') ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_notification_action (
    action_id                 INTEGER PRIMARY KEY,
    notification_type         TEXT NOT NULL
    CHECK ( notification_type IN ('REQUESTFORSTART', 'COORDINATORREQUESTFORVOTE', 'PARTICIPANTREQUESTFORVOTE', 'COMMIT', 'ABORT', 'MESSAGEDROPPED') ),
    dropped_message           TEXT
    CHECK ( (dropped_message IS NOT NULL) OR (notification_type != 'MESSAGEDROPPED') ),
    request_for_vote_value    BINARY
    CHECK ( (request_for_vote_value IS NOT NULL) OR (notification_type != 'PARTICIPANTREQUESTFORVOTE') ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_update_context_action_participant (
    action_id                 INTEGER PRIMARY KEY,
    process                   TEXT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') OR vote IS NULL ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE,
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_update_context_action(action_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_deliver_event (
    event_id                  INTEGER PRIMARY KEY,
    epoch                     BIGINT NOT NULL,
    receiver_service_id       TEXT NOT NULL,
    message_type              TEXT NOT NULL
    CHECK ( message_type IN ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT') ),
    vote_response             TEXT
    CHECK ( (vote_response IN ('TRUE', 'FALSE')) OR (message_type != 'VOTERESPONSE') ),
    vote_request              BINARY
    CHECK ( (vote_request IS NOT NULL) OR (message_type != 'VOTEREQUEST') ),
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_start_event (
    event_id                  INTEGER PRIMARY KEY,
    value                     BINARY,
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_vote_event (
    event_id                  INTEGER PRIMARY KEY,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') ),
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

-- Move data from the old tables into the updated tables
INSERT INTO consensus_2pc_update_context_action
    (
        action_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        vote,
        decision_timeout_start,
        action_alarm
    )
    SELECT
        action_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        vote,
        decision_timeout_start,
        action_alarm
    FROM old_consensus_2pc_update_context_action;

INSERT INTO consensus_2pc_send_message_action
    (
        action_id,
        epoch,
        receiver_service_id,
        message_type,
        vote_response,
        vote_request
    )
    SELECT
        action_id,
        epoch,
        receiver_service_id,
        message_type,
        vote_response,
        vote_request
    FROM old_consensus_2pc_send_message_action;

INSERT INTO consensus_2pc_notification_action
    (
        action_id,
        notification_type,
        dropped_message,
        request_for_vote_value
    )
    SELECT
        action_id,
        notification_type,
        dropped_message,
        request_for_vote_value
    FROM old_consensus_2pc_notification_action;

INSERT INTO consensus_2pc_update_context_action_participant
    (
        action_id,
        process,
        vote
    )
    SELECT
        action_id,
        process,
        vote
    FROM old_consensus_2pc_update_context_action_participant;

INSERT INTO consensus_2pc_deliver_event
    (
        event_id,
        epoch,
        receiver_service_id,
        message_type,
        vote_response,
        vote_request
    )
    SELECT
        event_id,
        epoch,
        receiver_service_id,
        message_type,
        vote_response,
        vote_request
    FROM old_consensus_2pc_deliver_event;

INSERT INTO consensus_2pc_start_event
    (
        event_id,
        value
    )
    SELECT
        event_id,
        value
    FROM old_consensus_2pc_start_event;

INSERT INTO consensus_2pc_vote_event
    (
        event_id,
        vote
    )
    SELECT
        event_id,
        vote
    FROM old_consensus_2pc_vote_event;
