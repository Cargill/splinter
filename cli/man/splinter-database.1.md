% SPLINTER-DATABASE(1) Cargill, Incorporated | Splinter Commands
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

**splinter-database** — Provides database management functions for Biome

SYNOPSIS
========

**splinter** **database** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for working with the Biome database.
(Biome provides user management functionality for Splinter applications.)
For example, the `migrate` subcommand updates the Biome database to a
new release.

FLAGS
=====

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decreases verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

SUBCOMMANDS
===========

`migrate`
: Updates the Biome database for a new Splinter release

SEE ALSO
========
| `splinter-database-migrate(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

