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

PRAGMA foreign_keys = off;

CREATE TABLE temp (
    identity     TEXT    NOT NULL,
    role_id      TEXT    NOT NULL,
    PRIMARY KEY(identity, role_id),
    FOREIGN KEY (role_id) REFERENCES rbac_roles(id) ON DELETE CASCADE
);

INSERT INTO temp SELECT identity, role_id FROM rbac_assignments;

DROP TABLE rbac_assignments;

ALTER TABLE temp RENAME TO rbac_assignments;

PRAGMA foreign_keys = on;
