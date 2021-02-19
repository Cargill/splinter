% TRANSACT-PLAYLIST-SUBMIT(1) Cargill, Incorporated | Transact Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**transact-paylist-submit** — Submits signed batches to targets from batch input

SYNOPSIS
========
**transact playlist submit** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command submits signed batches to one or more targets from batch input.
The batch input is expected to be length-delimited protobuf Batch messages,
which should also be pre-signed for submission to the distributed ledger.
The command will continue to submit the batches at the provided rate until
the source is exhausted.

The submit tool assumes the distributed ledger's REST API supports Cylinder
JWT authentication.

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
`-i, --input FILE`
: The source of batch transactions

`-k, --key PRIVATE-KEY-FILE`
: Specifies the full path to the private key file. The key will be used to
  sign the batches as well as generate a JWT for authentication.

`--rate RATE`
: How many batches to submit per second, ex `5`. (default: `1`)

`--targets TARGETS`
: Node URLS to submit batches to, combine groups with `;`. The URL should
  include all of the information required to append `/batches` to the end.

`-u, --update UPDATE `
: The time in seconds between updates. The command will log the success rate
  of submitting the HTTP requests. (default: `30`)

EXAMPLES
========
The following shows submitting a batch source against a Splinter circuit
`jEWSK-jdjSM` with scabbard services. A Scabbard service runs a sabre
transaction handler. The smallbank smart contract must already be submitted to
scabbard.

```
transact playlist submit\
  --input batches.txt \
  --key ./alice.priv \
  --rate 1  \
  --target "http://0.0.0.0:8089/scabbard/XOHZe-GE1oY/a001"
```


SEE ALSO
========
| `transact(1)`
| `transact-playlist(1)`
| `transact-playlist-batch(1)`
| `transact-playlist-create(1)`
| `transact-playlist-process(1)`
|
| Transact documentation: https://docs.rs/transact/latest
