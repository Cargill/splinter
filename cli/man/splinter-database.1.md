% SPLINTER-DATABASE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-database** — Provides database management functions for Biome

SYNOPSIS
========

**splinter** **database** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for working with the Splinter daemon database.
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
: Updates the database for a new Splinter release

SEE ALSO
========
| `splinter-database-migrate(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
