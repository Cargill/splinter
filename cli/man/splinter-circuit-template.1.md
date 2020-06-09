% SPLINTER-CIRCUIT-TEMPLATE(1) Cargill, Incorporated | Splinter Commands
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

**splinter-circuit-template** — Manage circuit templates

SYNOPSIS
========
**splinter circuit template** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command provides subcommands to list the available circuit templates, display
template details, and show the required arguments for a specific template.

Circuit templates help simplify the process of creating new circuits with the
`splinter circuit propose` command. A circuit template specifies the required
arguments and rules for the circuit. Each template on the system must have a unique
name.

A Scabbard circuit template, named `scabbard`, is available by default (packaged
with the Splinter CLI). This template can be used as a model for other circuit
templates.

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
`arguments`
: List arguments of a template.

`list`
: List available templates.

`show`
: Display a specific available template.

SEE ALSO
========
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
