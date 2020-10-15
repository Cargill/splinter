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

CREATE TABLE IF NOT EXISTS circuit_proposal (
    proposal_type             TEXT NOT NULL,
    circuit_id                TEXT PRIMARY KEY,
    circuit_hash              TEXT NOT NULL,
    requester                 BINARY NOT NULL,
    requester_node_id         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS vote_record (
    circuit_id                TEXT NOT NULL,
    public_key                BINARY NOT NULL,
    vote                      TEXT NOT NULL,
    voter_node_id             TEXT NOT NULL,
    PRIMARY KEY (circuit_id, voter_node_id),
    FOREIGN KEY (circuit_id) REFERENCES circuit_proposal(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proposed_circuit (
    circuit_id                TEXT NOT NULL,
    authorization_type        TEXT NOT NULL,
    persistence               TEXT NOT NULL,
    durability                TEXT NOT NULL,
    routes                    TEXT NOT NULL,
    circuit_management_type   TEXT NOT NULL,
    application_metadata      BINARY NOT NULL,
    comments                  TEXT NOT NULL,
    PRIMARY KEY (circuit_id),
    FOREIGN KEY (circuit_id) REFERENCES circuit_proposal(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proposed_node (
    circuit_id                TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (circuit_id, node_id),
    FOREIGN KEY (circuit_id) REFERENCES proposed_circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proposed_node_endpoint (
    node_id                TEXT NOT NULL,
    endpoint               TEXT NOT NULL,
    circuit_id             TEXT NOT NULL,
    PRIMARY KEY (node_id, endpoint),
    FOREIGN KEY (circuit_id, node_id) REFERENCES proposed_node(circuit_id, node_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proposed_service (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    service_type              TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (circuit_id, service_id),
    FOREIGN KEY (circuit_id) REFERENCES proposed_circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proposed_service_argument (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    key                       TEXT NOT NULL,
    value                     TEXT NOT NULL,
    PRIMARY KEY (circuit_id, service_id, key),
    FOREIGN KEY (circuit_id) REFERENCES proposed_circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS circuit (
    circuit_id                TEXT PRIMARY KEY,
    authorization_type        TEXT NOT NULL,
    persistence               TEXT NOT NULL,
    durability                TEXT NOT NULL,
    routes                    TEXT NOT NULL,
    circuit_management_type   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS service (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    service_type              TEXT NOT NULL,
    node_id                   TEXT NOT NULL,
    PRIMARY KEY (circuit_id, service_id),
    FOREIGN KEY (circuit_id) REFERENCES circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS service_argument (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    key                       TEXT NOT NULL,
    value                     TEXT NOT NULL,
    PRIMARY KEY (circuit_id, service_id, key),
    FOREIGN KEY (circuit_id) REFERENCES circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS circuit_member (
    circuit_id                TEXT NOT NULL,
    node_id                    TEXT NOT NULL,
    PRIMARY KEY (circuit_id, node_id),
    FOREIGN KEY (circuit_id) REFERENCES circuit(circuit_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS node_endpoint (
    node_id                TEXT NOT NULL,
    endpoint               TEXT NOT NULL,
    PRIMARY KEY (node_id, endpoint)
);
