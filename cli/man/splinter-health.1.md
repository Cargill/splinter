% SPLINTER-HEALTH(1) Cargill, Incorporated | Splinter Commands
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

**splinter-health** — Displays information about node and network health

SYNOPSIS
========

**splinter** **health** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========


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

`status`
: Displays information about a Splinter node (version, endpoint, node ID,
  and connected peers)

SEE ALSO
========
| `splinter-health-status(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

