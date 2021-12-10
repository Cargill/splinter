% SPLINTER-ROLE-DELETE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role-delete** — Deletes a role from a Splinter node

SYNOPSIS
========
**splinter role delete** \[**FLAGS**\] \[**OPTIONS**\] ROLE-ID

DESCRIPTION
===========
Deletes a role from a Splinter node.  This operation only effects the node
itself and not the wider network.

FLAGS
=====
`-n`, `--dry-run`
: Validate the command without performing the role deletion

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
`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.


ARGUMENTS
=========
`ROLE-ID`
: Specify the role ID of the role to be deleted.

EXAMPLES
========
This example removes a role from a given Splinter node.

* The role has ID `circuit_admin`.

```
$ splinter role delete \
  --url URL-of-splinterd-REST-API \
  circuit_admin
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role-create(1)`
| `splinter-role-update(1)`
| `splinter-role-list(1)`
| `splinter-role-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
