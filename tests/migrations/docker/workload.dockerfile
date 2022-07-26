# Copyright 2018-2022 Cargill Corporation
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
# ------------------------------------------------------------------------------

# amd64 is the only supported platform for transact
# hadolint ignore=DL3029
FROM --platform=linux/amd64 ubuntu:jammy

# hadolint ignore=DL3022
COPY --from=ghcr.io/hyperledger/transact-cli:0.4.6 /tmp/*.deb /tmp
# hadolint ignore=DL3022
COPY --from=splintercommunity/scabbard-cli:main /tmp/*.deb /tmp
# hadolint ignore=DL3022
COPY --from=splintercommunity/splinter-cli:main /tmp/*.deb /tmp

RUN apt-get update \
 && apt-get install -y -q --no-install-recommends \
    ca-certificates \
    curl \
    man \
 && dpkg --unpack /tmp/*.deb \
 && apt-get -f -y install --no-install-recommends \
 && rm -r /var/lib/apt/lists/*
