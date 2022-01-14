% SPLINTER-AUTHID-CREATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid-create** — Creates an authorized identity on a Splinter node

SYNOPSIS
========
**splinter authid create** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Creates an authorized identity which specifies a set of roles assigned to a
given identity, either a public key or a user ID, for accessing the REST API on
a Splinter node. This operation only effects the node itself and not the wider
network.

FLAGS
=====
`-n`, `--dry-run`
: Validate the command without authorizing the identity

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
: Specifies the user identity to authorize. Mutually exclusive to `--id-key`

`--id-key` PUBLIC-KEY
: Specifies the public key identity to authorize. Mutually exclusive to
  `--id-user`

`--role` ROLE-ID
: Specifies a role to be included in the assignment. Specify multiple times for
  more roles. At least one role is required.

EXAMPLES
========
This example creates an authorized identity with two assigned roles.

* The identity has user ID `"user-1234-abcd"`
* There exists two roles on the system: `circuit_reader` and `status_reader`

```
$ splinter authid create \
  --url URL-of-splinterd-REST-API \
  --role circuit_reader \
  --role status_reader \
  --id-user user-1234-abcd
```

This can be verified by using the `authid show` command:

```
$ splinter authid show \
  --url URL-of-splinterd-REST-API \
  --id-user user-1234-abcd
ID: user-1234-abcd
    Type: user
    roles:
        circuit_reader
        status_reader
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-authid-delete(1)`
| `splinter-authid-list(1)`
| `splinter-authid-show(1)`
| `splinter-authid-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
