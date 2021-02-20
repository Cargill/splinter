# Copyright 2018-2020 Cargill Incorporated
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
    client \
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
            cmd="cargo build --tests --manifest-path=$crate/Cargo.toml $feature"
            echo "\033[1m$cmd\033[0m"
            $cmd
        done
    done
    echo "\n\033[92mBuild Success\033[0m\n"

ci:
    just ci-lint-client
    just ci-lint-splinter
    just ci-test
    just ci-test-gameroom
    just ci-test-gameroom-ui

ci-lint-client:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build lint-gameroom-client
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-gameroom-client

ci-lint-splinter:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build \
      lint-splinter clippy-splinter
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-splinter
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit clippy-splinter

ci-test:
    #!/usr/bin/env sh
    set -e
    docker-compose -f tests/test-splinter.yaml build unit-test-splinter
    docker-compose -f tests/test-splinter.yaml up \
      --abort-on-container-exit unit-test-splinter

ci-test-gameroom: test-gameroom

ci-test-gameroom-ui: test-gameroom-ui

clean:
    cargo clean

copy-env:
    #!/usr/bin/env sh
    set -e
    find . -name .env | xargs -I '{}' sh -c "echo 'Copying to {}'; rsync .env {}"

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

test: build
    #!/usr/bin/env sh
    set -e
    for feature in $(echo {{features}})
    do
        for crate in $(echo {{crates}})
        do
            cmd="cargo test --manifest-path=$crate/Cargo.toml $feature"
            echo "\033[1m$cmd\033[0m"
            $cmd
        done
    done
    echo "\n\033[92mTest Success\033[0m\n"

test-gameroom:
    #!/usr/bin/env sh
    set -e
    docker-compose -f examples/gameroom/tests/docker-compose.yaml build
    docker-compose -f examples/gameroom/tests/docker-compose.yaml up \
    --abort-on-container-exit

test-gameroom-ui:
    #!/usr/bin/env sh
    set -e
    docker-compose -f examples/gameroom/tests/cypress/docker-compose.yaml build
    docker-compose -f examples/gameroom/tests/cypress/docker-compose.yaml up \
    --abort-on-container-exit
