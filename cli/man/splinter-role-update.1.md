% SPLINTER-ROLE-UPDATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role-update** — Updates a role on a Splinter node

SYNOPSIS
========
**splinter role update** \[**FLAGS**\] \[**OPTIONS**\] ROLE-ID

DESCRIPTION
===========
Updates an existing role used for accessing the Splinter REST API. This command
allows the user to change the role's display name or set of permissions.

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
: Remove all of the permissions currently associated with the role.

`-f`, `--force`
: Ignore errors based on duplicate values or adding and removing the same
  permission.

OPTIONS
=======
`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

`-D`, `--display` DISPLAY-NAME
: Specifies the display name for the created role.

`--add-perm` PERMISSION
: Specifies a permission to be add to the role. Specify multiple times for
  more permissions.

`--rm-perm` PERMISSION
: Specifies a permission to be removed from the role. Specify multiple times for
  more permissions.

ARGUMENTS
=========
`ROLE-ID`
: Specify the role ID of the role to be updated.

EXAMPLES
========
This example updates the `circuit_admin` role on a splinter node.

* The role has ID `circuit_admin`.
* The role has a display name of `"Circuit Admin"`
* The role has the permissions circuit read and write
* The role will be updated to have the permission `status.read`


```
$ splinter role update \
  --url URL-of-splinterd-REST-API \
  --add-perm status.read \
  circuit_admin
```

This can be verified by using the `role show` command:

```
$ splinter role show \
  --url URL-of-splinterd-REST-API \
  circuit_admin
ID: circuit_admin
    Name: Circuit Administrator
    Permissions:
        circuit.read
        circuit.write
        status.read
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role-list(1)`
| `splinter-role-show(1)`
| `splinter-role-create(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
