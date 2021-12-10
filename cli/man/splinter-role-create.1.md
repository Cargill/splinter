% SPLINTER-ROLE-CREATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-role-create** — Creates a role on a Splinter node

SYNOPSIS
========
**splinter role create** \[**FLAGS**\] \[**OPTIONS**\] ROLE-ID

DESCRIPTION
===========
Creates a role that specifies a set of permissions for accessing the REST API
on a Splinter node. This operation only effects the node itself and not the
wider network.

FLAGS
=====
`-n`, `--dry-run`
: Validate the command without performing the role creation

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

`-D`, `--display` DISPLAY-NAME
: Specifies the display name for the created role. This is a required option.

`-P`, `--permission` PERMISSION
: Specifies a permission to be included in the role. Specify multiple times for
  more permissions. At least one permission is required.


ARGUMENTS
=========
`ROLE-ID`
: Specify the role ID of the role to be created.

EXAMPLES
========
This example creates an `circuit_admin` role on a splinter node.

* The role has ID `circuit_admin`.
* The role has a display name of `"Circuit Admin"`
* The role has permissions to read and write circuits


```
$ splinter role create \
  --url URL-of-splinterd-REST-API \
  --permission circuit.read \
  --permission circuit.write \
  --display "Circuit Admin" \
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
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role-update(1)`
| `splinter-role-delete(1)`
| `splinter-role-list(1)`
| `splinter-role-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
