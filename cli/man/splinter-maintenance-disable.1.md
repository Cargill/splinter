% SPLINTER-MAINTENANCE-DISABLE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-maintenance-disable** — Disables maintenance mode for a Splinter node

SYNOPSIS
========

**splinter maintenance disable** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========

Maintenance mode may be used to temporarily disable write operations for the
Splinter REST API. This command disables maintenance mode for a particular
Splinter node.

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

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys) for authenticating with the Splinter REST
  API.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
This example shows how to disable maintenance mode for the Splinter node at
`http://localhost:8080`:

```
$ splinter maintenance disable -U http://localhost:8080
Maintenance mode has been disabled
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-maintenance-status(1)`
| `splinter-maintenance-enable(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
