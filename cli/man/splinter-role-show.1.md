% SPLINTER-ROLE-SHOW(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role-show** — Displays information about a role

SYNOPSIS
========
**splinter role show** \[**FLAGS**\] \[**OPTIONS**\] ROLE-ID

DESCRIPTION
===========
Display the entire definition of a role. This definition includes the set of
permissions allowed by the role.

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
: Specifies the output format of the role proposal. (default `human`).
  Possible values for formatting are `human`, `json`, or `yaml`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.


ARGUMENTS
=========
`ROLE-ID`
: Specify the role ID of the role to be shown.

EXAMPLES
========
This command displays information about a role with the default `human`
formatting, which intends to use indentation and labels to make the role
information understandable.

* The role has ID `circuit_admin`.

The information displayed below is local to the node where the role has been
defined.

```
$ splinter role show \
  --url URL-of-splinterd-REST-API \
  circuit_admin
ID: circuit_admin
    Name: Circuit Administrator
    Permissions:
        circuit.read
        circuit.write
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role-list(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
