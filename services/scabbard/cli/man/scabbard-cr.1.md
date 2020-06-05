% SCABBARD-CR(1) Cargill, Incorporated | Splinter Commands
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

**scabbard-cr** — Provides management of the Sabre contract registry.

SYNOPSIS
========

**scabbard cr** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command provides management functionality for the Sabre contract registry
of a scabbard service.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

SUBCOMMANDS
===========
`create`
: Creates a contract registry in a scabbard service's state.

`delete`
: Deletes a contract registry from a scabbard service's state.

`update`
: Updates the owner(s) of an existing contract registry in a scabbard service's
  state.

SEE ALSO
========
| `scabbard-cr-create(1)`
| `scabbard-cr-delete(1)`
| `scabbard-cr-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
