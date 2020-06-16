% SPLINTER-HEALTH(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-health** — Displays information about node and network health

SYNOPSIS
========

**splinter** **health** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========


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


SUBCOMMANDS
===========

`status`
: Displays information about a Splinter node (version, endpoint, node ID,
  and connected peers)

SEE ALSO
========
| `splinter-health-status(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

