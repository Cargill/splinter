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
    vote                      TEXT
    CHECK ( (vote IN ('TRUE' , 'FALSE')) OR ( state != 'VOTED') ),
    decision_timeout_start    BIGINT
    CHECK ( (decision_timeout_start IS NOT NULL) OR ( state != 'VOTED') ),
    ack_timeout_start         BIGINT
    CHECK ( (ack_timeout_start IS NOT NULL) OR ( state != 'WAITING_FOR_DECISION_ACK') ),
    PRIMARY KEY (circuit_id, service_id),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);


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
        decision_timeout_start
    )
    SELECT
        circuit_id,
        service_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        vote,
        decision_timeout_start
    FROM consensus_2pc_context;


DROP TABLE consensus_2pc_context;

ALTER TABLE new_consensus_2pc_context RENAME TO consensus_2pc_context;

CREATE TABLE IF NOT EXISTS new_scabbard_peer (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    peer_service_id           TEXT,
    PRIMARY KEY(circuit_id, service_id, peer_service_id)
    FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

INSERT INTO new_scabbard_peer
    (
        circuit_id,
        service_id,
        peer_service_id
    )
    SELECT
        circuit_id,
        service_id,
        peer_service_id
    FROM scabbard_peer;

DROP TABLE scabbard_peer;

ALTER TABLE new_scabbard_peer RENAME TO scabbard_peer;

CREATE TABLE IF NOT EXISTS new_scabbard_v3_commit_history (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    epoch                     INTEGER NOT NULL,
    value                     TEXT NOT NULL,
    decision                  TEXT,
    CHECK ( decision IN ('COMMIT', 'ABORT') ),
    PRIMARY KEY (circuit_id, service_id, epoch),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

INSERT INTO new_scabbard_v3_commit_history
    (
        circuit_id,
        service_id,
        epoch,
        value,
        decision
    )
    SELECT
        circuit_id,
        service_id,
        epoch,
        value,
        decision
    FROM scabbard_v3_commit_history;

DROP TABLE scabbard_v3_commit_history;

ALTER TABLE new_scabbard_v3_commit_history RENAME TO scabbard_v3_commit_history;


CREATE TABLE IF NOT EXISTS new_consensus_2pc_context_participant (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    process                   TEXT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') OR vote IS NULL ),
    decision_ack NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (circuit_id, service_id, process),
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
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

DROP TABLE consensus_2pc_context_participant;

ALTER TABLE new_consensus_2pc_context_participant RENAME TO consensus_2pc_context_participant;

CREATE TABLE IF NOT EXISTS new_consensus_2pc_event (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    event_type                TEXT NOT NULL
    CHECK ( event_type IN ('ALARM', 'DELIVER', 'START', 'VOTE') ), 
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE
);

INSERT INTO new_consensus_2pc_event
    (
        id,
        circuit_id,
        service_id,
        created_at,
        executed_at,
        event_type
    )
    SELECT
        id,
        circuit_id,
        service_id,
        created_at,
        executed_at,
        event_type
    FROM consensus_2pc_event;

DROP TABLE consensus_2pc_event;

ALTER TABLE new_consensus_2pc_event RENAME TO consensus_2pc_event;

CREATE TABLE IF NOT EXISTS new_scabbard_alarm (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    alarm_type                TEXT NOT NULL
    CHECK ( alarm_type IN ('TWOPHASECOMMIT')),
    alarm                     BIGINT NOT NULL,
    FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE,
    PRIMARY KEY (circuit_id, service_id, alarm_type)
);

INSERT INTO new_scabbard_alarm
    (
        circuit_id,
        service_id,
        alarm_type,
        alarm
    )
    SELECT
        circuit_id,
        service_id,
        alarm_type,
        alarm
    FROM scabbard_alarm;

DROP TABLE scabbard_alarm;

ALTER TABLE new_scabbard_alarm RENAME TO scabbard_alarm;

PRAGMA foreign_keys=on;
