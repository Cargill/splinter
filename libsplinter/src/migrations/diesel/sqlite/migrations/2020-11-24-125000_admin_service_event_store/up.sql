---- Copyright 2018-2020 Cargill Incorporated
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

CREATE TABLE IF NOT EXISTS admin_event_entry (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    circuit_id               TEXT NOT NULL,
    event_type               TEXT NOT NULL,
    data                     BINARY,
    timestamp                TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS admin_event_circuit_proposal (
    event_id                  INTEGER NOT NULL,
    proposal_type             TEXT NOT NULL,
    circuit_id                TEXT NOT NULL,
    circuit_hash              TEXT NOT NULL,
    requester                 BINARY NOT NULL,
    requester_node_id         TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_entry(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_vote_record (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    public_key                BINARY NOT NULL,
    vote                      TEXT NOT NULL,
    voter_node_id             TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id, voter_node_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_create_circuit (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    authorization_type        TEXT NOT NULL,
    persistence               TEXT NOT NULL,
    durability                TEXT NOT NULL,
    routes                    TEXT NOT NULL,
    circuit_management_type   TEXT NOT NULL,
    application_metadata      BINARY NOT NULL,
    comments                  TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_proposed_node (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id, node_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_proposed_node_endpoint (
    event_id               INTEGER NOT NULL,
    node_id                TEXT NOT NULL,
    endpoint               TEXT NOT NULL,
    circuit_id             TEXT NOT NULL,
    PRIMARY KEY (event_id, node_id, endpoint),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_proposed_service (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    service_type              TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id, service_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_proposed_service_argument (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    key                       TEXT NOT NULL,
    value                     TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id, service_id, key),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_event_proposed_service_node (
    event_id                  INTEGER NOT NULL,
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (event_id, circuit_id, service_id, node_id),
    FOREIGN KEY (event_id) REFERENCES admin_event_circuit_proposal(event_id) ON DELETE CASCADE
);
