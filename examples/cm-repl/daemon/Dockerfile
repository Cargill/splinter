# Copyright 2019 Cargill Incorporated
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

# -------------=== Build connection manager daemon ===-------------

FROM splintercommunity/splinter-dev as DAEMON_BUILDER

# Copy over source files
COPY Cargo.toml /build/Cargo.toml
COPY protos /build/protos
COPY examples/cm-repl /build/examples/cm-repl
COPY libsplinter /build/libsplinter
COPY protos/ /build/protos

# Build the project
WORKDIR /build/examples/cm-repl/daemon
ARG REPO_VERSION
RUN sed -i -e "0,/version.*$/ s/version.*$/version\ =\ \"${REPO_VERSION}\"/" Cargo.toml
RUN cargo deb --deb-version $REPO_VERSION

# -------------=== Build connection manager cli ===-------------

FROM splintercommunity/splinter-dev as CLI_BUILDER

# Copy over source files
COPY Cargo.toml /build/Cargo.toml
COPY protos /build/protos
COPY examples/cm-repl /build/examples/cm-repl
COPY libsplinter /build/libsplinter
COPY protos/ /build/protos

# Build the project
WORKDIR /build/examples/cm-repl/cli
ARG REPO_VERSION
RUN sed -i -e "0,/version.*$/ s/version.*$/version\ =\ \"${REPO_VERSION}\"/" Cargo.toml
RUN cargo deb --deb-version $REPO_VERSION

# -------------=== cm-repl docker build ===-------------
FROM ubuntu:bionic

COPY --from=DAEMON_BUILDER /build/target/debian/cm-repl*.deb /tmp
COPY --from=CLI_BUILDER /build/target/debian/cmc*.deb /tmp

RUN apt-get update \
 && dpkg --unpack /tmp/cm-repl*.deb \
 && dpkg --unpack /tmp/cmc*.deb \
 && apt-get -f -y install

CMD ["cm-repl-daemon"]
