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
    role_id      TEXT    NOT NULL,
    permission   TEXT    NOT NULL,
    PRIMARY KEY(role_id, permission),
    FOREIGN KEY (role_id) REFERENCES rbac_roles(id) ON DELETE CASCADE
);

INSERT INTO temp SELECT role_id,permission FROM rbac_role_permissions;

DROP TABLE rbac_role_permissions;

ALTER TABLE temp RENAME TO rbac_role_permissions;

PRAGMA foreign_keys = on;
