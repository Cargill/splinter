-- Copyright 2018-2021 Cargill Incorporated
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

ALTER TABLE identities RENAME TO _identities_old;

CREATE TABLE identities (
    identity      TEXT PRIMARY KEY,
    identity_type TEXT CHECK( identity_type IN ('key','user') ) NOT NULL
);

INSERT INTO identities (identity, identity_type)
  SELECT identity, (case
                      when 1 then 'key'
                      when 2 then 'user'
                      -- default to user, as this is the arbitrary value
                      else 'user'
                    end)
  FROM _identities_old;

DROP TABLE _identities_old;
