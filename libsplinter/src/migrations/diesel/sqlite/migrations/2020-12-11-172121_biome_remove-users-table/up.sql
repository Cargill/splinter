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

-- Rename the existing tables to old tables.
ALTER TABLE user_credentials RENAME TO old_user_credentials;
ALTER TABLE keys RENAME TO old_keys;
ALTER TABLE refresh_tokens RENAME TO old_refresh_tokens;
ALTER TABLE oauth_user RENAME TO old_oauth_user;

-- Recreate the tables without the foreign key constraints
CREATE TABLE IF NOT EXISTS  user_credentials (
  id                        INTEGER         PRIMARY KEY AUTOINCREMENT,
  user_id                   TEXT            NOT NULL,
  username                  TEXT            NOT NULL,
  password                  TEXT            NOT NULL
);
CREATE TABLE IF NOT EXISTS keys (
    public_key            TEXT NOT NULL,
    encrypted_private_key TEXT NOT NULL,
    user_id               TEXT NOT NULL,
    display_name          TEXT NOT NULL,
    PRIMARY KEY(public_key, user_id)
);
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id                    INTEGER       PRIMARY KEY AUTOINCREMENT,
    user_id               TEXT          NOT NULL,
    token                 TEXT          NOT NULL
);
CREATE TABLE IF NOT EXISTS oauth_user (
    id                  INTEGER       PRIMARY KEY AUTOINCREMENT,
    user_id             TEXT          NOT NULL,
    provider_user_ref   TEXT          NOT NULL,
    access_token        TEXT,
    refresh_token       TEXT,
    provider_id         INTEGER       NOT NULL,
    last_updated        TIMESTAMP     DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Move records to the new, relaxed tables
INSERT INTO user_credentials
    (
        id,
        user_id,
        username,
        password
    )
    SELECT
        id,
        user_id,
        username,
        password
    FROM old_user_credentials;
INSERT INTO keys
    (
        public_key,
        encrypted_private_key,
        user_id,
        display_name
    )
    SELECT
        public_key,
        encrypted_private_key,
        user_id,
        display_name
    FROM old_keys;
INSERT INTO refresh_tokens
    (
        id,
        user_id,
        token
    )
    SELECT
        id,
        user_id,
        token
    FROM old_refresh_tokens;
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

--  Drop the old indexes and tables
DROP TABLE old_user_credentials;
DROP TABLE old_keys;
DROP TABLE old_refresh_tokens;
DROP INDEX idx_oauth_user_access_token;
DROP INDEX idx_oauth_user_provider_user_ref;
DROP TABLE old_oauth_user;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_oauth_user_access_token ON oauth_user (
    access_token
);
CREATE INDEX IF NOT EXISTS idx_oauth_user_provider_user_ref ON oauth_user (
    provider_user_ref
);

DROP TABLE splinter_user;
