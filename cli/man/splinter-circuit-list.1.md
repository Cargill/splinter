% SPLINTER-CIRCUIT-LIST(1) Cargill, Incorporated | Splinter Commands
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

**splinter-circuit-list** — Displays the existing circuits for this Splinter node

SYNOPSIS
========
**splinter circuit list** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command lists all or some of the circuits the local node is a member of.
This command displays abbreviated information pertaining to circuits in columns,
with the headers `ID`, `MANAGEMENT` and `MEMBERS`. This makes it possible to
verify that circuits have been successfully created as well as being able to
access the generated circuit ID assigned to a circuit. The information displayed
will be the same for all member nodes. The circuits listed have been accepted by
all members.

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
`-f`, `--format` FORMAT
: Specifies the output format of the circuit. (default `human`). Possible values
  for formatting are `human` and `csv`.

`-m`, `--member` <member>
: Filter the circuits list by a node ID that is present in the circuits’ members
  list.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
This command displays information about circuits with a default `human`
formatting, meaning the information is displayed in a table. The `--member` option
allows for filtering the circuits.

The following command does not specify any filters, therefore all circuits
the local node, `alpha-node-000` is a member of are displayed.
```
$ splinter circuit list \
  --url URL-of-alpha-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
01234-ABCDE   mgmt001       alpha-node-000;beta-node-000
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
56789-ABCDE   mgmt002       alpha-node-000;gamma-node-000
```

The next command specifies a `--member` filter, therefore all circuits
the local node, `alpha-node-000` is a part of including the `gamma-node-000` node
ID will be listed.
```
$ splinter circuit list \
  member gamma-node-000 \
  --url URL-of-alpha-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
56789-ABCDE   mgmt002       alpha-node-000;gamma-node-000
```

Since all of the circuits listed have been accepted by each member, the same
circuit information will be displayed for member nodes.

From the perspective of the `gamma-node-000` node, this command will display the
following with no filters:
```
$ splinter circuit list \
  --url URL-of-gamma-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
56789-ABCDE   mgmt002       alpha-node-000;gamma-node-000
```

From the perspective of the `beta-node-000` node, this command will display the
following with no filters:
```
$ splinter circuit list \
  --url URL-of-gamma-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
01234-ABCDE   mgmt001       alpha-node-000;beta-node-000
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-show(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
