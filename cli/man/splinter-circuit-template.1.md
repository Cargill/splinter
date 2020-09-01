% SPLINTER-CIRCUIT-TEMPLATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-template** — Manage circuit templates

SYNOPSIS
========
**splinter circuit template** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command provides subcommands to list the available circuit templates, display
template details, and show the required arguments for a specific template.

Circuit templates help simplify the process of creating new circuits with the
`splinter circuit propose` command. A circuit template specifies the required
arguments and rules for the circuit. Each template on the system must have a unique
name.

A Scabbard circuit template, named `scabbard`, is available by default (packaged
with the Splinter CLI). This template can be used as a model for other circuit
templates.

All available templates are located in the default circuit templates directory,
`/usr/share/splinter/circuit-templates`, unless `SPLINTER_CIRCUIT_TEMPLATE_PATH`
is set. Note, if multiple template storage directories are specified in the
`SPLINTER_CIRCUIT_TEMPLATE_PATH`, they are searched from first to last for
template files. The first file matching the specified `TEMPLATE-NAME` will
be displayed.

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
`arguments`
: List arguments of a template.

`list`
: List available templates.

`show`
: Display a specific available template.

ENVIRONMENT VARIABLES
=====================
**SPLINTER_CIRCUIT_TEMPLATE_PATH**
: Paths containing circuit template files. Multiple values may be provided,
  separated by `:`, using the format `DIR1:DIR2:DIR3`. If multiple directories
  are specified, the directories are searched from first to last for template
  files.

SEE ALSO
========
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
