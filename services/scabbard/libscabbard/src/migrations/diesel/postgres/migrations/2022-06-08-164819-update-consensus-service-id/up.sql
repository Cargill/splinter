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
ALTER TABLE consensus_2pc_context RENAME TO old_consensus_2pc_context;
ALTER TABLE consensus_2pc_context_participant RENAME TO old_consensus_2pc_context_participant;
ALTER TABLE consensus_2pc_event RENAME TO old_consensus_2pc_event;

-- Recreate the tables with the circuit_id column
CREATE TABLE IF NOT EXISTS consensus_2pc_context (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    coordinator               TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    last_commit_epoch         BIGINT,
    state                     context_state NOT NULL,
    vote_timeout_start        BIGINT
    CHECK ( (vote_timeout_start IS NOT NULL) OR ( state != 'VOTING') ),
    vote                      TEXT
    CHECK ( (vote IN ('TRUE' , 'FALSE')) OR ( state != 'VOTED') ),
    decision_timeout_start    BIGINT
    CHECK ( (decision_timeout_start IS NOT NULL) OR ( state != 'VOTED') ),
    PRIMARY KEY (circuit_id, service_id)
);

CREATE TABLE IF NOT EXISTS consensus_2pc_context_participant (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    process                   TEXT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') OR vote IS NULL ),
    PRIMARY KEY (circuit_id, service_id, process)
);

CREATE TABLE IF NOT EXISTS consensus_2pc_event (
    id                        BIGSERIAL PRIMARY KEY,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    event_type                event_type NOT NULL
);

-- Move data from the old tables into the updated tables, splitting service_id into its two parts
-- circuit_id and service_id
INSERT INTO consensus_2pc_context
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
        split_part(service_id, '::', 1) circuit_id,
        split_part(service_id, '::', 2) service_id,
        coordinator,
        epoch,
        last_commit_epoch,
        state,
        vote_timeout_start,
        vote,
        decision_timeout_start
    FROM old_consensus_2pc_context;

INSERT INTO consensus_2pc_context_participant
    (
        circuit_id,
        service_id,
        epoch,
        process,
        vote
    )
    SELECT
        split_part(service_id, '::', 1) circuit_id,
        split_part(service_id, '::', 2) service_id,
        epoch,
        process,
        vote
    FROM old_consensus_2pc_context_participant;

INSERT INTO consensus_2pc_event
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
        split_part(service_id, '::', 1) circuit_id,
        split_part(service_id, '::', 2) service_id,
        created_at,
        executed_at,
        event_type
    FROM old_consensus_2pc_event;
