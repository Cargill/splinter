-- Copyright 2018-2020 Cargill Incorporated
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

CREATE TABLE IF NOT EXISTS roles (
    id           TEXT    PRIMARY KEY,
    display_name TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id      TEXT    NOT NULL,
    permission   TEXT    NOT NULL,
    PRIMARY KEY(role_id, permission),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS identities (
    identity      TEXT    PRIMARY KEY,
    identity_type INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS assignments (
    identity     TEXT    NOT NULL,
    role_id      TEXT    NOT NULL,
    PRIMARY KEY(identity, role_id),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
);
