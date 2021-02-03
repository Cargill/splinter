% SPLINTER-AUTHID-UPDATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid-update** — Updates an authorized identity on a Splinter node

SYNOPSIS
========
**splinter authid update** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Updates an existing authorized identity used for accessing the Splinter REST
API. This command allows the user to change the identity's set of roles.

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

`--rm-all`
: Remove all of the roles currently associated with the authorized identity.

`-f`, `--force`
: Ignore errors based on duplicate values or adding and removing the same
  role.

OPTIONS
=======
`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

`--id-user` USER-ID
: Specifies the user identity to update. Mutually exclusive to `--id-key`

`--id-key` PUBLIC-KEY
: Specifies the public key identity to update. Mutually exclusive to `--id-user`

`--add-role` ROLE-ID
: Specifies a role to be added to the authorized identity. Specify multiple
  times for more roles.

`--rm-role` ROLE-ID
: Specifies a role to be removed from the authorized identity. Specify multiple
  times for more roles.


EXAMPLES
========
This example updates an authorized identity with two assigned roles to remove
one of the roles and include a new role.

* The identity has user ID `"user-1234-abcd"`
* The identity is currently assigned roles `circuit_reader` and `status_reader`
* There exists three roles on the system: `circuit_reader`, `status_reader`, and
  `circuit_admin`

```
$ splinter authid update \
  --url URL-of-splinterd-REST-API \
  --id-user user-1234-abcd \
  --rm-role circuit_reader \
  --add-role circuit_admin
```

This can be verified by using the `authid show` command:

```
$ splinter authid show \
  --url URL-of-splinterd-REST-API \
  circuit_admin
ID: circuit_admin
    Name: Circuit Administrator
    Permissions:
        circuit_admin
        status_read
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-authid-create(1)`
| `splinter-authid-delete(1)`
| `splinter-authid-list(1)`
| `splinter-authid-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
