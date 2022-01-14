% SPLINTER-REGISTRY-ADD(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-registry-add** — Add a node to the local registry

SYNOPSIS
========

**splinter registry add** \[**FLAGS**\] \[**OPTIONS**\] IDENTITY

DESCRIPTION
===========

Add a new node to the local node registry. The node may be entirely new to the
registry, or it may be copied from the remote registries with the
`--from-remote` flag. If the `--from-remote` flag is used the `--display-name`,
`--endpoint`, `--key` and `--metadata` options may not be used to alter the
node being copied from the remote registry. When run, the command will
display the resulting changes as confirmation.

FLAGS
=====
`--dry-run`
: Shows the expected changes without submitting the node.

`--from-remote`
: Copies an existing node definition from the remote registries.

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

`--display-name` DISPLAY_NAME
: Sets a human-readable name for the new node. If not provided, a default value
based on the node's ID will be used.

`--endpoint ENDPOINT`
: Adds a network endpoint for the new node. At least one endpoint must be
provided, and all endpoints must be non-empty and unique in the registry (two
nodes cannot share the same endpoint). Repeat this option to specify multiple
endpoints.

`--key-file KEY`
: Add the public key to the new node. At least one key must be provided, and all
keys must be non-empty. Repeat this option to specify multiple keys.

`-k`, `--key KEY`
: Name or path of private key to be used for REST API authorization.

`--metadata METADATA_STRING`
: Adds the metadata to the new node, using the format
`METADATA_KEY:METADATA_VALUE`. If an entry for the given `METADATA_KEY` already
exists, it will be replaced. Repeat this option to specify multiple metadata
entries.

`-U`, `--url URL`
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

ARGUMENTS
=========

`IDENTITY`
Identity of the new node. Must be unique in the local registry.

EXAMPLES
========

The simplest use of this command is to create a new node with an identity, a
single endpoint, and a single key:

```
splinter registry add example-node-1 \
  --endpoint tcps://splinterd-node-1:8044 \
  --key /path/to/public/key/file \
  --url http://splinterd-rest-api:8085
```

Multiple endpoints, keys, and metadata entries can be provided by specifying the
arguments multiple times:

```
splinter registry add example-node-2 \
  --endpoint tcps://splinterd-node-2:8044 \
  --endpoint tcp://splinterd-node-2:8045 \
  --key /path/to/public/key/file1 \
  --key /path/to/public/key/file2 \
  --metadata key1:value1 \
  --metadata key2:value2 \
  --url http://splinterd-rest-api:8085
```

A node that exists in one or more remote registries can be copied to the local
registry with just the node's identity and the `--from-remote` flag, the
`--display-name`, `--endpoint`, `--key` and `--metadata` options may not be used
with this flag:

```
splinter registry add example-node-3 \
  --from-remote \
  --url http://splinterd-rest-api:8085
```

ENVIRONMENT VARIABLES
=====================

**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-registry-build(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
