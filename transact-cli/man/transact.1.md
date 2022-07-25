% Transact(1) Cargill, Incorporated | Transact Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**transact** — Command-line interface for Transact

SYNOPSIS
========

**transact** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

The `transact` utility is the command-line interface for Transact, a
transaction execution platform designed to be used as a library. This CLI
provides a tool for running workloads against distributed ledgers.

* Run `transact --help` to see the list of subcommands.

* Run `transact *SUBCOMMAND* --help` to see information about a specific
  subcommand (for example, `transact workload --help`).

* To view the man page for a Transact subcommand, use the "dashed form" of the
  name, where each space is replaced with a hyphen. For example, run
  `man transact-workload` to see the man page for `transact workload`.

SUBCOMMANDS
===========

`workload`
: Provides a command to run workloads against a distributed ledger.

FLAGS
=====

Most `transact` subcommands accept the following common flags:

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Do not display output

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of `-q`). Specify multiple times for more
  output.

SEE ALSO
========
| `transact-workload(1)`
|
| Transact documentation: https://docs.rs/transact/latest
