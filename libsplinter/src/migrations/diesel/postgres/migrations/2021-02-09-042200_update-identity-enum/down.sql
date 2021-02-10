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

ALTER TABLE identities
   RENAME COLUMN identity_type TO id_type_enum;

ALTER TABLE identities
   ADD COLUMN identity_type INTEGER;

UPDATE identities SET identity_type = (case
                      when id_type_enum = 'key' then 1
                      when id_type_enum = 'user' then 2
                      -- default to user, as this is the arbitrary value
                      else 2
                    end);

ALTER TABLE identities
    ALTER COLUMN identity_type set NOT NULL;

ALTER TABLE identities
    DROP COLUMN id_type_enum;
