% SPLINTER-CIRCUIT-TEMPLATE-LIST(1) Cargill, Incorporated | Splinter Commands
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

EXAMPLES
========
The following example lists the circuit templates on a system that has only the
`scabbard` template, which is available by default (packaged with the Splinter CLI).

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
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
