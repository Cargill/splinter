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

group "default" {
    targets = [
    "scabbard-cli",
    "splinter-cli",
    "splinterd",
    ]
}

# --== variables ==--

variable "CARGO_ARGS" {
    default = ""
}

variable "DISTRO" {
    default = "jammy"
}

variable "ISOLATION_ID" {
    default = "latest"
}

variable "REPO_VERSION" {
    default = "0.7.1-dev"
}

target "all" {
    args = {
        CARGO_ARGS = "${CARGO_ARGS}"
        REPO_VERSION = "${REPO_VERSION}"
    }
    platforms = ["linux/amd64", "linux/arm64"]
}

# --== splinter services ==--

target "scabbard-cli" {
    inherits = ["all"]
    dockerfile = "services/scabbard/cli/Dockerfile-installed-${DISTRO}"
    tags = ["ghcr.io/splintercommunity/scabbard-cli:${ISOLATION_ID}"]
}

target "splinter-cli" {
    inherits = ["all"]
    dockerfile = "cli/Dockerfile-installed-${DISTRO}"
    tags = ["ghcr.io/splintercommunity/splinter-cli:${ISOLATION_ID}"]
}

target "splinterd" {
    inherits = ["all"]
    dockerfile = "splinterd/Dockerfile-installed-${DISTRO}"
    tags = ["ghcr.io/splintercommunity/splinterd:${ISOLATION_ID}"]
}
