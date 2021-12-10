% SPLINTER-REGISTRY(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-registry** — Splinter registry commands.

SYNOPSIS
========

**splinter** **registry** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for updating the Splinter registry.

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

`add`
: Add a node to the local registry

`build`
: Add a node to a YAML file

SEE ALSO
========
| `splinter-registry-add(1)`
| `splinter-registry-build(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
