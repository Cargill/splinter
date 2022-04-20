# Copyright 2018-2022 Cargill Incorporated
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

FROM splintercommunity/splinter-dev:v11

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    postgresql-client \
 && rm -r /var/lib/apt/lists/*

COPY Cargo.toml /build/Cargo.toml
COPY README.md /build/README.md
COPY libsplinter /build/libsplinter
COPY splinterd /build/splinterd
COPY cli /build/cli
COPY rest_api/actix_web_1/ /build/rest_api/actix_web_1/
COPY rest_api/actix_web_4/Cargo.toml /build/rest_api/actix_web_4/Cargo.toml
COPY rest_api/common/ /build/rest_api/common/
COPY services/scabbard/cli /build/services/scabbard/cli
COPY services/scabbard/libscabbard /build/services/scabbard/libscabbard
COPY examples/gameroom/database /build/examples/gameroom/database
COPY examples/gameroom/daemon /build/examples/gameroom/daemon
COPY examples/gameroom/cli /build/examples/gameroom/cli

COPY justfile /build

WORKDIR /build
