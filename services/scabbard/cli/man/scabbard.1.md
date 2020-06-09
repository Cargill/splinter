% SCABBARD(1) Cargill, Incorporated | Splinter Commands

NAME
====

**scabbard** — Command-line interface for scabbard

SYNOPSIS
========

**scabbard** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

The `scabbard` utility is the command-line interface for scabbard, a Splinter
service that runs Sawtooth Sabre smart contracts on Hyperledger Transact. This
CLI is a convenient tool for uploading, viewing, and executing smart contracts.

* Run `scabbard *SUBCOMMAND* --help` to see information about a specific
  subcommand (for example, `scabbard contract upload --help`).

* To view the man page for a scabbard subcommand, use the "dashed form" of the
  name, where each space is replaced with a hyphen. For example, run
  `man scabbard-contract-show` to see the man page for `scabbard contract show`.

FLAGS
=====

`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

SUBCOMMANDS
===========

`contract`
: Provides commands to upload, list, and show Sabre smart contracts.

`cr`
: Provides commands to create, update, and delete a Sabre contract registry.

`exec`
: Executes a Sabre smart contract.

`ns`
: Provides commands to create, update, and delete Sabre namespaces.

`perm`
: Sets or deletes a Sabre namespace permission.

SEE ALSO
========
| `scabbard-contract-list(1)`
| `scabbard-contract-show(1)`
| `scabbard-contract-upload(1)`
| `scabbard-cr-create(1)`
| `scabbard-cr-delete(1)`
| `scabbard-cr-update(1)`
| `scabbard-exec(1)`
| `scabbard-ns-create(1)`
| `scabbard-ns-delete(1)`
| `scabbard-ns-update(1)`
| `scabbard-perm(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
