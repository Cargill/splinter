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

PRAGMA foreign_keys=off;

-- Rename the existing table to the old table.
ALTER TABLE oauth_user RENAME TO old_oauth_user;

CREATE TABLE oauth_user (
    id                  INTEGER       PRIMARY KEY AUTOINCREMENT,
    user_id             TEXT          NOT NULL,
    provider_user_ref   TEXT          NOT NULL,
    access_token        TEXT,
    refresh_token       TEXT,
    provider_id         INTEGER       NOT NULL,
    last_updated        TIMESTAMP     DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (user_id) REFERENCES splinter_user(id) ON DELETE CASCADE
);

-- Move the records to the new, relaxed table.
INSERT INTO oauth_user
    (
        id,
        user_id,
        provider_user_ref,
        access_token,
        refresh_token,
        provider_id
    )
    SELECT
        id,
        user_id,
        provider_user_ref,
        access_token,
        refresh_token,
        provider_id
    FROM old_oauth_user;

--  Drop the old indexes and table
DROP INDEX idx_oauth_user_access_token;
DROP INDEX idx_oauth_user_provider_user_ref;
DROP TABLE old_oauth_user;


-- Recreate the indexes
CREATE INDEX IF NOT EXISTS idx_oauth_user_access_token ON oauth_user (
    access_token
);

CREATE INDEX IF NOT EXISTS idx_oauth_user_provider_user_ref ON oauth_user (
    provider_user_ref
);

PRAGMA foreign_keys=on;
