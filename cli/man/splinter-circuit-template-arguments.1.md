% SPLINTER-CIRCUIT-TEMPLATE-ARGUMENTS(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-template-arguments** — Displays the arguments defined in a
circuit template

SYNOPSIS
========
**splinter circuit template arguments** \[**FLAGS**\] TEMPLATE-NAME

DESCRIPTION
===========
Circuit templates help simplify the process of creating new circuits with the
`splinter circuit propose` command. Circuit template arguments are required when
building a circuit from the template. This command lists the arguments that are
defined in the specified circuit template.

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

ARGUMENTS
=========
`TEMPLATE-NAME`
: Name of the circuit template containing the arguments of interest. The
  template file must exist in the specified circuit template directory.
  The circuit template directory is by default
  `/usr/share/splinter/circuit-templates`, unless
  `SPLINTER_CIRCUIT_TEMPLATE_PATH` is set.

ENVIRONMENT VARIABLES
=====================
**SPLINTER_CIRCUIT_TEMPLATE_PATH**
: Paths containing circuit template files. Multiple values may be provided,
  separated by `:`, using the format `DIR1:DIR2:DIR3`. If multiple directories
  are specified, the directories are searched from first to last for template
  files.

EXAMPLES
========
The following command shows the arguments for the `scabbard` circuit template,
which is available by default (packaged with the Splinter CLI).

```
$ splinter circuit template arguments scabbard

name: admin_keys
required: false
default_value: $(a:SIGNER_PUB_KEY)
description: Public keys used to verify transactions in the scabbard service

name: nodes
required: true
default_value: Not set
description: List of node IDs

name: signer_pub_key
required: false
default_value: Not set
description: Public key of the signer
```

SEE ALSO
========
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
