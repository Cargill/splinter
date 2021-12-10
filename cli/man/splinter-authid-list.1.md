% SPLINTER-AUTHID-LIST(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid-list** — Displays the existing authorized identities for this
Splinter node

SYNOPSIS
========
**splinter authid list** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command lists all of the authorized identities with assigned roles the
local node has configured. This command displays abbreviated information
pertaining to assignments in columns, with the headers `ID`, `TYPE`, and
`ROLES`. This allows the user to quickly see which identities have been assigned
roles, as well as how many. The information displayed is only relevant to the
queried splinter node.

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
: Specifies the output format of the list. (default `human`). Possible values
  for formatting are `human` and `csv`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
This command displays information about authorized identities with a default
`human` formatting, meaning the information is displayed in a table.  In this
example, the node is currently configured with a single user identity and a
single public key identity. The user is assigned to 2 roles; the key is assigned
to 1

```
$ splinter role list \
  --url URL-of-splinterd-REST-API
IDENTITY                                                           TYPE ROLES
6596ee05-0997-5897-87be-566c0984f2ec                               user 2
03d4a6ea6bae775622912b6cf49437098dc3bf06ca49ea331113e27ee0b14c7a3c key  1
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| Splinter documentation: https://www.splinter.dev/docs/0.7/
