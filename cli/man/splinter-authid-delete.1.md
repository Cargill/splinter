% SPLINTER-AUTHID-DELETE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid-delete** — Deletes an authorized identity on a Splinter node

SYNOPSIS
========
**splinter authid delete** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Deletes an existing authorized identity used for accessing the Splinter REST
API.

FLAGS
=====
`-n`, `--dry-run`
: Validate the command without deleting the identity's authorizations

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

`--id-user` USER-ID
: Specifies the user identity to delete. Mutually exclusive to `--id-key`

`--id-key` PUBLIC-KEY
: Specifies the public key identity to delete. Mutually exclusive to `--id-user`

EXAMPLES
========
This example deletes an authorized identity with two assigned roles.

* The identity has user ID `"user-1234-abcd"`

```
$ splinter authid delete \
  --url URL-of-splinterd-REST-API \
  --id-user user-1234-abcd \
```

This can be verified by using the `authid list` command, which will no longer
list the identity.

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-authid-create(1)`
| `splinter-authid-list(1)`
| `splinter-authid-show(1)`
| `splinter-authid-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
