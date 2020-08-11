% SPLINTER-CIRCUIT-PROPOSALS(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-proposals** — Lists the current circuit proposals

SYNOPSIS
========
**splinter circuit proposals** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Lists all of the circuit proposals that the local node is a proposed member of.
This command displays abbreviated information pertaining to proposed circuits in
columns, with the headers `ID`, `MANAGEMENT` and `MEMBERS`. This makes it possible
to verify that circuit proposals have been successfully proposed as well as being
able to access the generated circuit ID assigned to a proposal. Circuit proposals
have not necessarily been voted on by all proposed members.

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
: Specifies the output format of the circuit proposal. (default `human`).
  Possible values for formatting are `human` and `csv`. The `human` option
  displays the circuit proposals information in a formatted table, while `csv`
  prints the circuit proposals information via comma-separated values.

`--management-type` MANAGEMENT-TYPE
: Filter the circuit proposals by their circuit management type.

`-m`, `--member` MEMBER
: Filter the circuits list by a node ID that is present in the circuit proposal’s
  members list.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
This command displays information about circuit proposals with a default `human`
formatting, meaning the information is displayed in a table. The `--member` and
`--management-type` options allow for filtering the circuit proposals.

The following command does not specify any filters, therefore all circuit proposals
the local node, `alpha-node-000` is a part of are displayed.
```
$ splinter circuit proposals \
  --url URL-of-alpha-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
01234-ABCDE   mgmt001       alpha-node-000;beta-node-000
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
56789-ABCDE   mgmt002       alpha-node-000;gamma-node-000
```

The next command specifies a `--management-type` filter, therefore all circuit
proposals the local node, `alpha-node-000` is a part of with a `circuit_management_type`
of `mgmt001` will be listed.
```
$ splinter circuit proposals \
  --management-type mgmt001 \
  --url URL-of-alpha-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
01234-ABCDE   mgmt001       alpha-node-000;beta-node-000
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
```

The next command specifies a `--member` filter, therefore all circuit proposals
the local node, `alpha-node-000` is a part of including the `gamma-node-000` node
ID will be listed.
```
$ splinter circuit proposals \
  member gamma-node-000 \
  --url URL-of-alpha-node-splinterd-REST-API
ID            MANAGEMENT    MEMBERS
43210-ABCDE   mgmt001       alpha-node-000;gamma-node-000
56789-ABCDE   mgmt002       alpha-node-000;gamma-node-000
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-vote(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
