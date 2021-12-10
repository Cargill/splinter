% SPLINTER-ROLE-LIST(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role-list** — Displays the existing roles for this Splinter node

SYNOPSIS
========
**splinter role list** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command lists all of the roles the local node has available. This command
displays abbreviated information pertaining to roles in columns, with the
headers `ID` and `DISPLAY NAME`. This makes it possible to verify that
roles have been successfully created as well as being able to access the
available role ID for use when assigning a role to an identity. The information
displayed is only relevant to the queried splinter node.

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
This command displays information about roles with a default `human`
formatting, meaning the information is displayed in a table.

```
$ splinter role list \
  --url URL-of-splinterd-REST-API
ID             NAME
circuit_admin  Circuit Administrator
circuit_reader Circuit Reader
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role-create(1)`
| `splinter-role-update(1)`
| `splinter-role-delete(1)`
| `splinter-role-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
