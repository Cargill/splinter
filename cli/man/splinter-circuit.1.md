% SPLINTER-CIRCUIT(1) Cargill, Incorporated | Splinter Commands
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

**splinter-circuit** — Provides circuit management functionality.

SYNOPSIS
========
**splinter circuit** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command allows nodes to create and manage circuits and circuit proposals.
Commands to list and display circuits and circuit proposals that a node is a member
of are also available subcommands. Nodes are also able to vote to accept or reject
circuit proposals using the `splinter-circuit-vote` subcommand.

FLAGS
=====
`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

SUBCOMMANDS
===========
`default`
: Manage default values for circuit creation.

`list`
: List all circuits that have been accepted by all proposed members.

`proposals`
: List all circuit proposals. Circuit proposals have not been voted on by all
  proposed members.

`propose`
: Propose a new circuit to be created.

`show`
: Display a specific circuit or circuit proposal.

`template`
: Manage circuit templates used for circuit creation.

`vote`
: Vote on a new circuit proposal. Only the proposed members that did not propose
  the circuit are able to vote on a circuit. The circuit requester has an assumed
  vote of `ACCEPT`.

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-show(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
