# Copyright 2018-2021 Cargill Incorporated
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

# build stage
FROM node:lts-alpine as build-stage

RUN apk update \
 && apk add \
    curl \
    g++ \
    git \
    make \
    python \
 && rm -rf /var/cache/apk/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh \
    | sh -s -- --to /usr/local/bin

COPY examples/gameroom/gameroom-app/package*.json examples/gameroom/gameroom-app/

WORKDIR /examples/gameroom/gameroom-app

# Need to set as non-root user to properly install transact-sdk-javascript
RUN npm config set unsafe-perm true && npm install

WORKDIR /

COPY examples/gameroom/gameroom-app examples/gameroom/gameroom-app
COPY justfile .
