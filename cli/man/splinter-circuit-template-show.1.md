% SPLINTER-CIRCUIT-TEMPLATE-SHOW(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
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
