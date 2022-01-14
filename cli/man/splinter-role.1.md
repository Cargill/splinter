% SPLINTER-ROLE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role** — Provides management functions for the Role-Based
Authorization configuration of a Splinter node.

SYNOPSIS
========

**splinter** **role** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for viewing and modifying role-based access
permissions for a Splinter daemon.

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

`create`
: Creates a role on a Splinter node

`delete`
: Deletes a role from a splinter node

`list`
: Lists the available roles for a Splinter node

`show`
: Shows a role on a Splinter node

`update`
: Updates a role on a Splinter node

SEE ALSO
========
| `splinter-role-create(1)`
| `splinter-role-update(1)`
| `splinter-role-delete(1)`
| `splinter-role-list(1)`
| `splinter-role-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
