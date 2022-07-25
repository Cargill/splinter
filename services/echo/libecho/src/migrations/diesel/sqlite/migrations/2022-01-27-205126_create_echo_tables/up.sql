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

CREATE TABLE IF NOT EXISTS echo_services (
    service_id  TEXT PRIMARY KEY NOT NULL,
    frequency   INTEGER,
    jitter      INTEGER,
    error_rate  FLOAT,
    status      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS echo_peers (
    service_id       TEXT NOT NULL,
    peer_service_id  TEXT,
    PRIMARY KEY(service_id, peer_service_id),
    FOREIGN KEY(service_id) REFERENCES echo_services(service_id)
);

CREATE TABLE IF NOT EXISTS echo_requests (
    sender_service_id    TEXT NOT NULL,
    correlation_id       BIGINT NOT NULL UNIQUE,
    receiver_service_id  TEXT NOT NULL,
    message              TEXT NOT NULL,
    sent                 INTEGER NOT NULL,
    sent_at              INTEGER,
    ack                  INTEGER NOT NULL,
    ack_at               INTEGER,
    PRIMARY KEY(sender_service_id, correlation_id),
    FOREIGN KEY(sender_service_id) REFERENCES echo_services(service_id)
);

CREATE INDEX IF NOT EXISTS idx_echo_requests_correlation_id ON echo_requests (
    correlation_id
);

CREATE TABLE IF NOT EXISTS echo_request_errors (
    service_id      TEXT NOT NULL,
    correlation_id  BIGINT NOT NULL UNIQUE,
    error_message   TEXT NOT NULL,
    error_at        INTEGER NOT NULL,
    PRIMARY KEY(service_id, correlation_id),
    FOREIGN KEY(service_id) REFERENCES echo_services(service_id)
);

CREATE INDEX IF NOT EXISTS idx_echo_request_errors_correlation_id ON echo_request_errors (
    correlation_id
);
