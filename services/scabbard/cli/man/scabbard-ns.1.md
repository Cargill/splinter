% SCABBARD-NS(1) Cargill, Incorporated | Splinter Commands

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
