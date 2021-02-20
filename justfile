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

crates := '\
    libsplinter \
    splinterd \
    cli \
    services/scabbard/cli \
    services/scabbard/libscabbard \
    services/health \
    examples/gameroom/database \
    examples/gameroom/daemon \
    examples/gameroom/cli \
    '

features := '\
    --features=experimental \
    --features=stable \
    --features=default \
    --no-default-features \
    '

build:
    #!/usr/bin/env sh
    set -e
    for feature in $(echo {{features}})
    do
        for crate in $(echo {{crates}})
        do
            cmd="cargo build --tests --manifest-path=$crate/Cargo.toml $BUILD_MODE $feature"
            echo "\033[1m$cmd\033[0m"
            $cmd
        done
    done
    echo "\n\033[92mBuild Success\033[0m\n"

ci:
    just lint-client-ci
    just lint-splinter-ci
    just test-ci
    just test-gameroom-ci

clean:
    cargo clean

lint:
    #!/usr/bin/env sh
    set -e
    echo "\033[1mcargo fmt -- --check\033[0m"
    cargo fmt -- --check
    for feature in $(echo {{features}})
    do
        for crate in $(echo {{crates}})
        do
            cmd="cargo clippy --manifest-path=$crate/Cargo.toml $feature -- -D warnings"
            echo "\033[1m$cmd\033[0m"
            $cmd
        done
    done
    echo "\n\033[92mLint Success\033[0m\n"

lint-client:
    #!/usr/bin/env sh
    set -e
    cd examples/gameroom/gameroom-app
    npm run lint

lint-client-ci:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build lint-gameroom-client
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-gameroom-client

lint-splinter-ci:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build lint-splinter
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-splinter

test: build
    #!/usr/bin/env sh
    set -e
    for feature in $(echo {{features}})
    do
        for crate in $(echo {{crates}})
        do
            cmd="cargo test --manifest-path=$crate/Cargo.toml $TEST_MODE $feature"
            echo "\033[1m$cmd\033[0m"
            $cmd
        done
    done
    echo "\n\033[92mTest Success\033[0m\n"

test-ci:
    #!/usr/bin/env sh
    set -e
    docker-compose -f tests/test-splinter.yaml build unit-test-splinter
    docker-compose -f tests/test-splinter.yaml up \
      --abort-on-container-exit unit-test-splinter

test-gameroom:
    #!/usr/bin/env sh
    set -e
    docker-compose -f examples/gameroom/tests/docker-compose.yaml build
    docker-compose -f examples/gameroom/tests/docker-compose.yaml up \
    --abort-on-container-exit

test-gameroom-ci: test-gameroom
