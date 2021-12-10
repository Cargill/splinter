% SPLINTER-REGISTRY-BUILD(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-registry-build** — Add a node to a YAML file

SYNOPSIS
========

**splinter registry build** \[**FLAGS**\] \[**OPTIONS**\] IDENTITY

DESCRIPTION
===========

Add a node to a YAML file that will be passed to a Splinter daemon as a part of
the registry. The parts of a node definition that are not passed to this command
are retrieved from the `STATUS_URL`.

FLAGS
=====
`--force`
: Overwrite node if it already exists

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decreases verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======

`--file` FILE
: Path of registry file to add node to; defaults to './nodes.yaml'

`--key-file` PUBLIC_KEY
: Path of public key file to include with node

`--metadata` METADATA_STRING
:  Metadata to include with node (<key>=<value>)

`-k`, `--key KEY`
: Name or path of private key to be used for REST API authorization

ARGUMENTS
=========

`STATUS_URL`
URL of splinter REST API to query for node data

EXAMPLES
========
The following command adds a node for the Alpha node to the file at
`/registry/registry.yaml`

```
splinter registry build \
  http://splinterd-alpha:8085 \
  --file /registry/registry.yaml \
  --key-file /registry/alpha.pub \
  --metadata organization='Alpha'
```


ENVIRONMENT VARIABLES
=====================

**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `STATUS_URL`.)

SEE ALSO
========
| `splinter-registry-add(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.6/
