% SCABBARD-CONTRACT(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

NAME
====

**scabbard-contract** — Provides contract management functionality

SYNOPSIS
========

**scabbard contract** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command allows users to upload and view Sabre contracts for a scabbard
service.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

SUBCOMMANDS
===========
`list`
: Displays contracts that have already been uploaded to a scabbard service.

`show`
: Shows details about a specific smart contract that was uploaded to a scabbard
  service.

`upload`
: Uploads a smart contract to a scabbard service.

SEE ALSO
========
| `scabbard-contract-list(1)`
| `scabbard-contract-show(1)`
| `scabbard-contract-upload(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
