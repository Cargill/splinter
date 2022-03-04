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

FROM ubuntu:focal

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install base dependencies
RUN apt-get update \
 && apt-get install -y -q --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    g++ \
    gcc \
    git \
    libpq-dev \
    libsasl2-dev \
    libssl-dev \
    libsqlite3-dev \
    libzmq3-dev \
    openssl \
    pandoc \
    pkg-config \
    python \
    unzip \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*

ENV PATH=$PATH:/root/.cargo/bin

# Install Rust
RUN curl https://sh.rustup.rs -sSf > /usr/bin/rustup-init \
 && chmod +x /usr/bin/rustup-init \
 && rustup-init -y \
# Install cargo deb
 && cargo install cargo-deb \
# Install protoc
 && TARGET_ARCH=$(dpkg --print-architecture) \
 && if [[ $TARGET_ARCH == "arm64" ]]; then \
      PROTOC_ARCH="aarch_64"; \
    elif [[ $TARGET_ARCH == "amd64" ]]; then \
      PROTOC_ARCH="x86_64"; \
    fi \
 && curl -OLsS https://github.com/google/protobuf/releases/download/v3.7.1/protoc-3.7.1-linux-$PROTOC_ARCH.zip \
      && unzip -o protoc-3.7.1-linux-$PROTOC_ARCH.zip -d /usr/local \
      && rm protoc-3.7.1-linux-$PROTOC_ARCH.zip \
# Install just
 && curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Create empty cargo projects for top-level projects
WORKDIR /build
RUN USER=root cargo new --bin cli \
 && USER=root cargo new --lib libsplinter \
 && USER=root cargo new --lib rest_api/actix_web_1 \
 && USER=root cargo new --lib rest_api/actix_web_4 \
 && USER=root cargo new --lib rest_api/common \
 && USER=root cargo new --bin splinterd \
 && cp libsplinter/src/lib.rs splinterd/src/lib.rs \
# Create empty Cargo projects for gameroom
 && USER=root cargo new --bin examples/gameroom/cli \
 && USER=root cargo new --bin examples/gameroom/daemon \
 && USER=root cargo new --lib examples/gameroom/database \
# Create empty Cargo projects for splinter services
 && USER=root cargo new --bin services/scabbard/cli \
 && USER=root cargo new --lib services/scabbard/libscabbard

# Copy over splinter files
COPY Cargo.toml /build/Cargo.toml
COPY cli/Cargo.toml /build/cli/Cargo.toml
COPY libsplinter/build.rs /build/libsplinter/build.rs
COPY libsplinter/Cargo.toml /build/libsplinter/Cargo.toml
COPY libsplinter/protos /build/libsplinter/protos
COPY rest_api/actix_web_1/Cargo.toml /build/rest_api/actix_web_1/Cargo.toml
COPY rest_api/actix_web_4/Cargo.toml /build/rest_api/actix_web_4/Cargo.toml
COPY rest_api/common/Cargo.toml /build/rest_api/common/Cargo.toml
COPY splinterd/Cargo.toml /build/splinterd/Cargo.toml
COPY services/scabbard/cli/Cargo.toml /build/services/scabbard/cli/Cargo.toml
COPY services/scabbard/libscabbard/build.rs /build/services/scabbard/libscabbard/build.rs
COPY services/scabbard/libscabbard/Cargo.toml /build/services/scabbard/libscabbard/Cargo.toml
COPY services/scabbard/libscabbard/protos /build/services/scabbard/libscabbard/protos

# Copy over example Cargo.toml files
COPY examples/gameroom/cli/Cargo.toml \
     /build/examples/gameroom/cli/Cargo.toml
COPY examples/gameroom/daemon/Cargo.toml \
     /build/examples/gameroom/daemon/Cargo.toml
COPY examples/gameroom/database/Cargo.toml \
     /build/examples/gameroom/database/Cargo.toml

# Do release builds for each Cargo.toml
# Workaround for https://github.com/koalaman/shellcheck/issues/1894
#hadolint ignore=SC2016
RUN find ./*/ -name 'Cargo.toml' -print0 | \
    xargs -0 -I {} sh -c 'echo Building $1; cargo build --tests --release --manifest-path $1 --features=experimental' sh {} \
 && find ./*/ -name 'Cargo.toml' -print0 | \
    xargs -0 -I {} sh -c 'echo Building $1; cargo build --tests --release --manifest-path $1 --features=stable' sh {} \
 && find ./*/ -name 'Cargo.toml' -print0 | \
    xargs -0 -I {} sh -c 'echo Building $1; cargo build --tests --release --manifest-path $1 --features=default' sh {} \
 && find ./*/ -name 'Cargo.toml' -print0 | \
    xargs -0 -I {} sh -c 'echo Building $1; cargo build --tests --release --manifest-path $1 --no-default-features' sh {} \
# Clean up built files
 && find target/release -path target/release/.fingerprint -prune -false -o -name '*gameroom*' | xargs -I {} sh -c 'rm -rf $1' sh {} \
 && find target/release -path target/release/.fingerprint -prune -false -o -name '*scabbard*' | xargs -I {} sh -c 'rm -rf $1' sh {} \
 && find target/release -path target/release/.fingerprint -prune -false -o -name '*splinter*' | xargs -I {} sh -c 'rm -rf $1' sh {} \
# Clean up leftover files
find . -name 'Cargo.toml' -exec \
    sh -c 'x="$1"; rm "$x" ' sh {} \; \
 &&  rm /build/libsplinter/build.rs \
    /build/libsplinter/protos/* \
    /build/services/scabbard/libscabbard/build.rs \
    /build/services/scabbard/libscabbard/protos/*

# Log the commit hash
COPY .git/ /tmp/.git/
WORKDIR /tmp
RUN git rev-parse HEAD > /commit-hash
WORKDIR /build
