# Copyright 2019-2021 Cargill Incorporated
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

FROM ubuntu:bionic as builder

ENV TRANSACT_FORCE_PANDOC=true

RUN apt-get update && \
    apt-get install -y \
    curl \
    gcc \
    git \
    libsqlite3-dev \
    libssl-dev \
    libzmq3-dev \
    pandoc \
    pkg-config \
    protobuf-compiler \
    sqlite3 \
    unzip

RUN curl https://sh.rustup.rs -sSf > /usr/bin/rustup-init && \
    chmod +x /usr/bin/rustup-init && \
    rustup-init -y

ENV PATH=$PATH:/root/.cargo/bin

RUN cargo install cargo-deb

# Copy over dependencies and source files
COPY libtransact /build/libtransact
COPY README.md /build/README.md
COPY cli /build/cli

WORKDIR /build/cli

ARG REPO_VERSION
ARG CARGO_ARGS
RUN sed -i -e "0,/version.*$/ s/version.*$/version\ =\ \"${REPO_VERSION}\"/" Cargo.toml
RUN sed -i -e "0,/readme.*$/ s/readme.*$/readme\ =\ \"\/build\/README.md\"/" Cargo.toml
RUN cargo deb --deb-version $REPO_VERSION $CARGO_ARGS

RUN mv /build/cli/target/debian/transact-cli*.deb /tmp

# Log the commit hash
COPY .git/ /tmp/.git/
WORKDIR /tmp
RUN git rev-parse HEAD > /commit-hash

# -------------=== transact-cli docker build ===-------------

FROM ubuntu:bionic

ARG CARGO_ARGS
RUN echo "CARGO_ARGS = '$CARGO_ARGS'" > CARGO_ARGS

COPY --from=builder /tmp/transact-cli*.deb /tmp/
COPY --from=builder /commit-hash /commit-hash

RUN apt-get update \
 && dpkg --unpack /tmp/transact-cli*.deb \
 && apt-get -f -y install
