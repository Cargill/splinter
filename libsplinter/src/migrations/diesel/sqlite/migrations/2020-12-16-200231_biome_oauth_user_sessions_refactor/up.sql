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

-- Create the new tables
CREATE TABLE IF NOT EXISTS oauth_users (
  subject                   TEXT        PRIMARY KEY,
  user_id                   TEXT        NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS oauth_user_sessions (
  splinter_access_token     TEXT        PRIMARY KEY,
  subject                   TEXT        NOT NULL,
  oauth_access_token        TEXT        NOT NULL,
  oauth_refresh_token       TEXT,
  last_authenticated        INTEGER     DEFAULT (strftime('%s','now')) NOT NULL,
  FOREIGN KEY (subject) REFERENCES oauth_users(subject) ON DELETE CASCADE
);

-- Drop the old table and its indexes; the data is not migrated because each
-- session now needs a `splinter_access_token`
DROP TABLE IF EXISTS oauth_user;
DROP INDEX IF EXISTS idx_oauth_user_access_token;
DROP INDEX IF EXISTS idx_oauth_user_provider_user_ref;
