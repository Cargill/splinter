% SCABBARD-CR(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
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
