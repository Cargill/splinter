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

CREATE TABLE IF NOT EXISTS scabbard_alarm (
    service_id                TEXT NOT NULL,
    alarm_type                TEXT NOT NULL
    CHECK ( alarm_type IN ('TWOPHASECOMMIT')),
    alarm                     BIGINT NOT NULL,
    FOREIGN KEY (service_id) REFERENCES scabbard_service(service_id) ON DELETE CASCADE,
    PRIMARY KEY (service_id, alarm_type)
);
