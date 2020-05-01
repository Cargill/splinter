% SPLINTER(1) Cargill, Incorporated | Splinter Commands

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

`registry`
: Provides commands to create and manage Splinter registry information.

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
| `splinter-cert-generate(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-propose(1)`
| `splinter-circuit-show(1)`
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
| `splinter-circuit-vote(1)`
| `splinter-database-migrate(1)`
| `splinter-health-status(1)`
| `splinter-keygen(1)`
| 
| `splinterd(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

