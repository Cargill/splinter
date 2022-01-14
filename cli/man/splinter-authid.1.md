% SPLINTER-AUTHID(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid** — Provides management functions for the Role-Based
Authorization authorized identities on a Splinter node.

SYNOPSIS
========

**splinter** **authid** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for viewing and modifying role-based access
assignments for a Splinter daemon.

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
: Creates an authorized identity on a Splinter node

`delete`
: Deletes an authorized identity on a Splinter node

`list`
: Lists the authorized identities on a Splinter node

`show`
: Shows an authorized identity on a Splinter node

`update`
: Updates an authorized identity on a Splinter node

SEE ALSO
========
| `splinter-authid-create(1)`
| `splinter-authid-delete(1)`
| `splinter-authid-list(1)`
| `splinter-authid-show(1)`
| `splinter-authid-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
