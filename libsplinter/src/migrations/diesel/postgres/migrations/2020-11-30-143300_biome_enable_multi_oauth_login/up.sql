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

-- Drop the exising not null constraints
ALTER TABLE oauth_user DROP CONSTRAINT oauth_user_user_id_key;
ALTER TABLE oauth_user DROP CONSTRAINT oauth_user_provider_user_ref_key;

-- Add a last_updated column
ALTER TABLE oauth_user ADD COLUMN last_updated TIMESTAMP DEFAULT NOW();

-- Set the last_updated column to "now" for the existing records.
UPDATE oauth_user SET last_updated = now();

-- Ensure that this field is always set
ALTER TABLE oauth_user ALTER COLUMN last_updated SET NOT NULL;
