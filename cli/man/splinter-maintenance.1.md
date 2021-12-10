% SPLINTER-MAINTENANCE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-maintenance** — Provides management functions for the maintenance
mode of a Splinter node.

SYNOPSIS
========

**splinter** **maintenance** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for checking and modifying the maintenance
status of the Splinter daemon.

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

`status`
: Checks if maintenance mode is enabled for a Splinter node

`enable`
: Enables maintenance mode for a Splinter node

`disable`
: Disables maintenance mode for a Splinter node

SEE ALSO
========
| `splinter-maintenance-status(1)`
| `splinter-maintenance-enable(1)`
| `splinter-maintenance-disable(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
