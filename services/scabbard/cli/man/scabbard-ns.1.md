% SCABBARD-NS(1) Cargill, Incorporated | Splinter Commands
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

**scabbard-ns** — Provides management of Sabre namespaces.

SYNOPSIS
========

**scabbard ns** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command provides management functionality for the Sabre namespaces of a
scabbard service. A Sabre namespace is a reserved portion of state that
can be written to and read by one or more smart contracts. A contract must be
given permission to read or write to a namespace (see `scabbard-perm(1)` for
setting namespace permissions).

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

SUBCOMMANDS
===========
`create`
: Creates a namespace in a scabbard service's state.

`delete`
: Deletes a namespace from a scabbard service's state.

`update`
: Updates the owner(s) of an existing namespace in a scabbard service's state.

SEE ALSO
========
| `scabbard-ns-create(1)`
| `scabbard-ns-delete(1)`
| `scabbard-ns-update(1)`
| `scabbard-perm(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
