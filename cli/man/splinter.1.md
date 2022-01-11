% SPLINTER(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter** — Command-line interface for Splinter

SYNOPSIS
========

**splinter** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

The `splinter` utility is the command-line interface for Splinter, a
privacy-focused platform for distributed applications. This CLI provides a
unified tool for creating circuits, generating keys, and other functions for
managing a Splinter node.

* Run `splinter --help` to see the list of subcommands.

* Run `splinter *SUBCOMMAND* --help` to see information about a specific
  subcommand (for example, `splinter circuit vote --help`).

* To view the man page for a Splinter subcommand, use the "dashed form" of the
  name, where each space is replaced with a hyphen. For example, run
  `man splinter-circuit-list` to see the man page for `splinter circuit list`.

SUBCOMMANDS
===========

`authid`
: Role-based authorization role assignment commands

`cert`
: Generates insecure certificates for development with the `generate`
  subcommand

`circuit`
: Provides circuit creation and management functions with `list`, `propose`,
  `template`, `vote`, and other subcommands

`database`
: Provides database functions with the `migrate` subcommand

`health`
: Displays information about network health with the `status` subcommand

`keygen`
: Generates secp256k1 public/private keys

`maintenance`
: Maintenance mode commands

`permissions`
: Lists REST API permissions for a Splinter node

`registry`
: Provides commands to create and manage Splinter registry information.

`role`
: Role-based authorization role-related commands

`state`
: Commands to manage scabbard state

`upgrade`
: Upgrades splinter YAML state to database state

`user`
: Splinter user commands

FLAGS
=====

Most `splinter` subcommands accept the following common flags:

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Do not display output

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of `-q`). Specify multiple times for more
  output.

ENVIRONMENT VARIABLES
=====================

Many `splinter` subcommands accept the following environment variable:

**`SPLINTER_REST_API_URL`**
: Specifies the endpoint for the Splinter daemon (`splinterd`)
  if `-U` or `--url` is not used.

SEE ALSO
========
| `splinter-authid-create(1)`
| `splinter-authid-delete(1)`
| `splinter-authid-list(1)`
| `splinter-authid-show(1)`
| `splinter-authid-update(1)`
| `splinter-cert-generate(1)`
| `splinter-circuit-abandon(1)`
| `splinter-circuit-disband(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-propose(1)`
| `splinter-circuit-purge(1)`
| `splinter-circuit-remove-proposal(1)`
| `splinter-circuit-show(1)`
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
| `splinter-circuit-vote(1)`
| `splinter-database-migrate(1)`
| `splinter-health-status(1)`
| `splinter-keygen(1)`
| `splinter-maintenance-status(1)`
| `splinter-maintenance-enable(1)`
| `splinter-maintenance-disable(1)`
| `splinter-permissions(1)`
| `splinter-registry-add(1)`
| `splinter-registry-build(1)`
| `splinter-role-create(1)`
| `splinter-role-delete(1)`
| `splinter-role-list(1)`
| `splinter-role-show(1)`
| `splinter-role-update(1)`
| `splinter-state-migrate(1)`
| `splinter-upgrade(1)`
| `splinter-user(1)`
|
| `splinterd(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
