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

CREATE FUNCTION update_oauth_user_session_timestamp() RETURNS trigger AS $$
    BEGIN
      UPDATE oauth_user_sessions
      SET last_authenticated = extract(epoch from now())
      WHERE splinter_access_token = OLD.splinter_access_token;
    END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER oauth_user_sessions_timestamp_update
  AFTER UPDATE ON oauth_user_sessions
  FOR EACH ROW EXECUTE PROCEDURE update_oauth_user_session_timestamp();
