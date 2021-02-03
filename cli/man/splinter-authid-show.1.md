% SPLINTER-AUTHID-SHOW(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-authid-show** — Displays information about an authorized identity on
a Splinter node

SYNOPSIS
========
**splinter authid show** \[**FLAGS**\] \[**OPTIONS**\] ROLE-ID

DESCRIPTION
===========
Display the entire definition of an authorized identity. This definition
includes the set of roles assigned to the identity.

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
: Specifies the output format of the authorized identity. (default `human`).
  Possible values for formatting are `human`, `json`, or `yaml`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.


EXAMPLES
========
This example shows an authorized identity with two assigned roles.

* The identity has user ID `"user-1234-abcd"`
* There exists two roles on the system: `circuit_reader` and `status_reader`

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
| `splinter-authid-create(1)`
| `splinter-authid-delete(1)`
| `splinter-authid-list(1)`
| `splinter-authid-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
