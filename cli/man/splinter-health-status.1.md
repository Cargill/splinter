% SPLINTER-HEALTH-STATUS(1) Cargill, Incorporated | Splinter Commands

NAME
====

**splinter-health-status** — Displays information about a Splinter node

SYNOPSIS
========
**splinter health status** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========

**NOTE: Currently, this command does not display node information. Full
functionality is planned in an upcoming release.**

This command displays a Splinter node's version, endpoint, node ID, and the
endpoints of its connected peers (other nodes on the same circuit or circuits
as this node).

FLAGS
=====

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======

`-U`, `--url URL`
: Specifies the URL for the node of interest (the URL for the `splinterd`
  REST API on the node). This option is required unless `$SPLINTER_REST_API_URL`
  is set.

ENVIRONMENT VARIABLES
=====================

**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-list(1)`
| `splinter-circuit-show(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

