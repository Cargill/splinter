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

CREATE TABLE IF NOT EXISTS splinter_nodes (
    identity      TEXT  PRIMARY KEY,
    display_name  TEXT  NOT NULL
);

CREATE TABLE IF NOT EXISTS splinter_nodes_endpoints (
    identity      TEXT  NOT NULL,
    endpoint      TEXT  NOT NULL,
    PRIMARY KEY (identity, endpoint),
    FOREIGN KEY (identity) REFERENCES splinter_nodes(identity) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS splinter_nodes_keys (
    identity      TEXT  NOT NULL,
    key           TEXT  NOT NULL,
    PRIMARY KEY (identity, key),
    FOREIGN KEY (identity) REFERENCES splinter_nodes(identity) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS splinter_nodes_metadata (
    identity      TEXT  NOT NULL,
    key           TEXT  NOT NULL,
    value         TEXT  NOT NULL,
    PRIMARY KEY (identity, key),
    FOREIGN KEY (identity) REFERENCES splinter_nodes(identity) ON DELETE CASCADE
);
