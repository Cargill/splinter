% SPLINTER-CIRCUIT-TEMPLATE-SHOW(1) Cargill, Incorporated | Splinter Commands

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
: Circuit template to be displayed.

EXAMPLES
========
The following command shows the details of the `scabbard` circuit template,
which is available by default (packaged with the Splinter CLI).

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
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
