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

-- create new tables with numeric type for vote columns
CREATE TABLE IF NOT EXISTS new_consensus_2pc_context (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    coordinator               TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    last_commit_epoch         BIGINT,
    state                     TEXT NOT NULL
    CHECK ( state IN ('WAITINGFORSTART', 'VOTING', 'WAITINGFORVOTE', 'ABORT', 'COMMIT', 'WAITINGFORVOTEREQUEST', 'VOTED', 'WAITING_FOR_DECISION_ACK') ),
    vote_timeout_start        BIGINT
    CHECK ( (vote_timeout_start IS NOT NULL) OR ( state != 'VOTING') ),
    vote                      NUMERIC
    CHECK ( (vote IS NOT NULL) OR ( state != 'VOTED') ),
    decision_timeout_start    BIGINT
    CHECK ( (decision_timeout_start IS NOT NULL) OR ( state != 'VOTED') ),
    ack_timeout_start         BIGINT
    CHECK ( (ack_timeout_start IS NOT NULL) OR ( state != 'WAITING_FOR_DECISION_ACK') ),
    PRIMARY KEY (circuit_id, service_id),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_context_participant (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    process                   TEXT NOT NULL,
    vote                      NUMERIC,
    decision_ack              NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (circuit_id, service_id, process),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_deliver_event (
    event_id                  INTEGER PRIMARY KEY,
    epoch                     BIGINT NOT NULL,
    receiver_service_id       TEXT NOT NULL,
    message_type              TEXT NOT NULL
    CHECK ( message_type IN ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT', 'DECISION_ACK') ),
    vote_response             NUMERIC
    CHECK ( (vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE') ),
    vote_request              BINARY
    CHECK ( (vote_request IS NOT NULL) OR (message_type != 'VOTEREQUEST') ),
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_send_message_action (
    action_id                 INTEGER PRIMARY KEY,
    epoch                     BIGINT NOT NULL,
    receiver_service_id       TEXT NOT NULL,
    message_type              TEXT NOT NULL
    CHECK ( message_type IN ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT', 'DECISION_ACK') ),
    vote_response             NUMERIC
    CHECK ( (vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE') ),
    vote_request              BINARY
    CHECK ( (vote_request IS NOT NULL) OR (message_type != 'VOTEREQUEST') ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_update_context_action (
    action_id                 INTEGER PRIMARY KEY,
    coordinator               TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    last_commit_epoch         BIGINT,
    state                     TEXT NOT NULL
    CHECK ( state IN ('WAITINGFORSTART', 'VOTING', 'WAITINGFORVOTE', 'ABORT', 'COMMIT', 'WAITINGFORVOTEREQUEST', 'VOTED', 'WAITING_FOR_DECISION_ACK') ),
    vote_timeout_start        BIGINT
    CHECK ( (vote_timeout_start IS NOT NULL) OR ( state != 'VOTING') ),
    vote                      NUMERIC
    CHECK ( (vote IS NOT NULL) OR ( state != 'VOTED') ),
    decision_timeout_start    BIGINT
    CHECK ( (decision_timeout_start IS NOT NULL) OR ( state != 'VOTED') ),
    action_alarm  BIGINT,
    ack_timeout_start         BIGINT
    CHECK ( (ack_timeout_start IS NOT NULL) OR ( state != 'WAITING_FOR_DECISION_ACK') ),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_update_context_action_participant (
    action_id                 INTEGER NOT NULL,
    process                   TEXT NOT NULL,
    vote                      NUMERIC,
    decision_ack              NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (action_id, process),
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE,
    FOREIGN KEY (action_id) REFERENCES consensus_2pc_update_context_action(action_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS new_consensus_2pc_vote_event (
    event_id                  INTEGER PRIMARY KEY,
    vote                      NUMERIC,
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE
);

-- move data from the existing tables into the new tables
INSERT INTO new_consensus_2pc_context
    (
        circuit_id,
        service_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        vote,
        decision_timeout_start,
        ack_timeout_start
    )
    SELECT
        circuit_id,
        service_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        CASE vote
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
        decision_timeout_start,
        ack_timeout_start
    FROM consensus_2pc_context;

INSERT INTO new_consensus_2pc_context_participant
    (
        circuit_id,
        service_id,
        epoch,
        process,
        vote,
        decision_ack
    )
    SELECT
        circuit_id,
        service_id,
        epoch,
        process,
        CASE vote
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
        decision_ack
    FROM consensus_2pc_context_participant;

INSERT INTO new_consensus_2pc_deliver_event
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
        CASE vote_response
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
        vote_request
    FROM consensus_2pc_deliver_event;

INSERT INTO new_consensus_2pc_send_message_action
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
        CASE vote_response
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
        vote_request
    FROM consensus_2pc_send_message_action;

INSERT INTO new_consensus_2pc_update_context_action
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
        CASE vote
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
        decision_timeout_start,
        action_alarm
    FROM consensus_2pc_update_context_action;

INSERT INTO new_consensus_2pc_update_context_action_participant
    (
        action_id,
        process,
        vote,
        decision_ack
    )
    SELECT
        action_id,
        process,
        CASE vote
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END,
            decision_ack
    FROM consensus_2pc_update_context_action_participant;

INSERT INTO new_consensus_2pc_vote_event
    (
        event_id,
        vote
    )
    SELECT
        event_id,
        CASE vote
            WHEN 'FALSE' THEN 0
            WHEN 'TRUE' THEN 1
            ELSE NULL
            END
    FROM consensus_2pc_vote_event;

-- delete existing tables and rename the new tables
DROP TABLE consensus_2pc_context;
DROP TABLE consensus_2pc_context_participant;
DROP TABLE consensus_2pc_deliver_event;
DROP TABLE consensus_2pc_send_message_action;
DROP TABLE consensus_2pc_update_context_action;
DROP TABLE consensus_2pc_update_context_action_participant;
DROP TABLE consensus_2pc_vote_event;

ALTER TABLE new_consensus_2pc_context RENAME TO consensus_2pc_context;
ALTER TABLE new_consensus_2pc_context_participant RENAME TO consensus_2pc_context_participant;
ALTER TABLE new_consensus_2pc_deliver_event RENAME TO consensus_2pc_deliver_event;
ALTER TABLE new_consensus_2pc_send_message_action RENAME TO consensus_2pc_send_message_action;
ALTER TABLE new_consensus_2pc_update_context_action RENAME TO consensus_2pc_update_context_action;
ALTER TABLE new_consensus_2pc_update_context_action_participant RENAME TO consensus_2pc_update_context_action_participant;
ALTER TABLE new_consensus_2pc_vote_event RENAME TO consensus_2pc_vote_event;

PRAGMA foreign_keys=on;
