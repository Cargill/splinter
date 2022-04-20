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

# Build gameroom cli

WORKDIR /project/gameroom
COPY examples/gameroom/database ./database
COPY examples/gameroom/cli ./cli
WORKDIR /project/gameroom/cli
RUN cargo build

ENV PATH=$PATH:/project/gameroom/cli/target/debug/:/project/bin/

WORKDIR /project/splinter/
