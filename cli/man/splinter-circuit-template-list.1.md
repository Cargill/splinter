% SPLINTER-CIRCUIT-TEMPLATE-LIST(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-template-list** — Displays all available circuit templates

SYNOPSIS
========
**splinter circuit template list** \[**FLAGS**\]

DESCRIPTION
===========
Circuit templates help simplify the process of creating new circuits with the
`splinter circuit propose` command. This command lists all available circuit
templates.

A Scabbard circuit template is available by default (this template is packaged
with the Splinter CLI).

All available templates are located in the default circuit templates directory,
`/usr/share/splinter/circuit-templates`, unless `SPLINTER_CIRCUIT_TEMPLATE_PATH`
is set. Note, if multiple template storage directories are specified in the
`SPLINTER_CIRCUIT_TEMPLATE_PATH`, they are searched from first to last for
template files. The first file matching the specified `TEMPLATE-NAME` will
be displayed.

Tip: Use the `splinter circuit template arguments` command to see the required
arguments for a specific circuit template.

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
`-F`, `--format` FORMAT
: Specifies the output format of the circuit templates. (default `human`).
  Possible values for formatting are `human` and `csv`. The `human` option
  displays the circuit template file information in a formatted table, while
  `csv` prints the circuit template file information via comma-separated values.

ENVIRONMENT VARIABLES
=====================
**SPLINTER_CIRCUIT_TEMPLATE_PATH**
: Paths containing circuit template files. Multiple values may be provided,
  separated by `:`, using the format `DIR1:DIR2:DIR3`. If multiple directories
  are specified, the directories are searched from first to last for template
  files.

EXAMPLES
========
The following example lists the circuit templates on a system that has only the
`scabbard` template, which is available by default (packaged with the Splinter CLI)
in the circuit template directory, `/usr/share/splinter/circuit-templates`, unless
`SPLINTER_CIRCUIT_TEMPLATE_PATH` is set.

```
$ splinter circuit template list
Available templates:
scabbard
```

SEE ALSO
========
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
