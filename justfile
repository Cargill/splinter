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

crates_quick := '\
    libsplinter \
    splinterd \
    cli \
    services/scabbard/cli \
    services/scabbard/libscabbard \
    services/health \
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
    just ci-lint-client
    just ci-lint-dockerfiles
    just ci-lint-splinter
    just ci-shellcheck
    just ci-test
    just ci-test-gameroom

ci-lint-client:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build lint-gameroom-client
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-gameroom-client

ci-lint-dockerfiles: lint-dockerfiles

ci-lint-splinter:
    #!/usr/bin/env sh
    set -e
    docker-compose -f docker/compose/run-lint.yaml build lint-splinter
    docker-compose -f docker/compose/run-lint.yaml up \
      --abort-on-container-exit lint-splinter

ci-shellcheck:
    #!/usr/bin/env sh
    set -e
    docker run --rm koalaman/shellcheck:stable --version
    docker run -t --rm -v $(pwd):/mnt koalaman/shellcheck:stable \
      cli/packaging/ubuntu/completions/splinter

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

clean-metrics:
    docker-compose -f docker/metrics/docker-compose.yaml down -v

copy-env:
    #!/usr/bin/env sh
    set -e
    find . -name .env | xargs -I '{}' sh -c "echo 'Copying to {}'; rsync .env {}"

docker-build:
    #!/usr/bin/env sh
    set -e
    export VERSION=AUTO_STRICT
    export REPO_VERSION=$(./bin/get_version)
    docker-compose -f docker-compose-installed.yaml build

lint: lint-ignore
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

lint-dockerfiles:
    #!/usr/bin/env sh
    set -e
    docker pull -q hadolint/hadolint
    for dockerfile in $(find . -iname *dockerfile* -not -path '*/\.git*';)
    do
        echo "\033[1mLinting $dockerfile\033[0m"
        docker run \
          --rm \
          -i \
          -v $(pwd)/ci/hadolint.yaml:/.config/hadolint.yaml \
          hadolint/hadolint < $dockerfile
    done
    echo "\n\033[92mLint Dockerfile Success\033[0m\n"

lint-ignore:
    #!/usr/bin/env sh
    set -e
    diff -u .dockerignore .gitignore
    echo "\n\033[92mLint Ignore Files Success\033[0m\n"

metrics:
    docker-compose -f docker/metrics/docker-compose.yaml down;
    docker-compose \
        -f docker/metrics/docker-compose.yaml \
        up \
        -d \
        --build;

qbuild:
    #!/usr/bin/env sh
    set -e
    for crate in $(echo {{crates_quick}})
    do
        cmd="cargo build --manifest-path=$crate/Cargo.toml $BUILD_MODE --features=experimental"
        echo "\033[1m$cmd\033[0m"
        $cmd
    done
    echo "\n\033[92mBuild Success\033[0m\n"

qlint:
    #!/usr/bin/env sh
    set -e
    echo "\033[1mcargo fmt -- --check\033[0m"
    cargo fmt -- --check
    for crate in $(echo {{crates_quick}})
    do
        cmd="cargo clippy --manifest-path=$crate/Cargo.toml --features=experimental -- -D warnings"
        echo "\033[1m$cmd\033[0m"
        $cmd
    done
    echo "\n\033[92mLint Success\033[0m\n"

qtest:
    #!/usr/bin/env sh
    set -e
    for crate in $(echo {{crates_quick}})
    do
        cmd="cargo test --manifest-path=$crate/Cargo.toml --features=experimental"
        echo "\033[1m$cmd\033[0m"
        $cmd
    done
    echo "\n\033[92mTest Success\033[0m\n"

shellcheck:
    #!/usr/bin/env sh
    set -e
    shellcheck cli/packaging/ubuntu/completions/splinter
    echo "\n\033[92mShellcheck Success\033[0m\n"

test:
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
