% SPLINTER-MAINTENANCE-STATUS(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-maintenance-status** — Checks if maintenance mode is enabled for a
Splinter node

SYNOPSIS
========

**splinter maintenance status** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========

Maintenance mode may be used to temporarily disable write operations for the
Splinter REST API. This command checks whether or not maintenance mode is
enabled for a particular Splinter node.

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
This example shows that the targeted node (at `http://localhost:8080`) is not in
maintenance mode:

```
$ splinter maintenance status -U http://localhost:8080
Maintenance mode is currently disabled
```

This example shows that the targeted node (at `http://localhost:8081`) is in
maintenance mode:

```
$ splinter maintenance status -U http://localhost:8081
Maintenance mode is currently enabled
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-maintenance-enable(1)`
| `splinter-maintenance-disable(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
