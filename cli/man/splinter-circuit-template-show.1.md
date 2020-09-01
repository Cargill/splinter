% SPLINTER-CIRCUIT-TEMPLATE-SHOW(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-template-show** — Displays the details of a circuit template

SYNOPSIS
========
**splinter circuit template show** \[**FLAGS**\] TEMPLATE-NAME

DESCRIPTION
===========
Circuit templates help simplify the process of creating new circuits with the
`splinter circuit propose` command. This command displays the entire template
definition, including the arguments and rules, for the specified template.

All available templates are located in the default circuit templates directory,
`/usr/share/splinter/circuit-templates`, unless `SPLINTER_CIRCUIT_TEMPLATE_PATH`
is set. Note, if multiple template storage directories are specified in the
`SPLINTER_CIRCUIT_TEMPLATE_PATH`, they are searched from first to last for
template files. The first file matching the specified `TEMPLATE-NAME` will
be displayed.

Tip: Use the `splinter circuit template arguments` command to show only the
required arguments for a specific circuit template.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information.

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

ARGUMENTS
=========
`TEMPLATE-NAME`
: Name of the circuit template to be displayed. The template file must exist in
  the specified circuit template directory. The circuit template directory is by
  default `/usr/share/splinter/circuit-templates`, unless
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
The following command shows the details of the `scabbard` circuit template,
which is available by default (packaged with the Splinter CLI) in the default
circuit template directory, `/usr/share/splinter/circuit-templates`, unless
`SPLINTER_CIRCUIT_TEMPLATE_PATH` is set.

```
$ splinter circuit template show scabbard
---
version: v1
args:
  - name: “$(a:ADMIN_KEYS)”
    required: false
    default: “$(a:SIGNER_PUB_KEY)”
    description: Public keys used to verify transactions in the scabbard service
  - name: “$(a:NODES)”
    required: true
    description: List of node IDs
  - name: “$(a:SIGNER_PUB_KEY)”
    required: false
    description: Public key of the signer
rules:
  create-services:
    service-type: scabbard
    service-args:
      - key: admin_keys
        value:
          - "$(a:ADMIN_KEYS)"
      - key: peer_services
        value: "$(r:ALL_OTHER_SERVICES)"
    first-service: a000
```

SEE ALSO
========
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
