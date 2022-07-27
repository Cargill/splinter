% SPLINTER-PLAYLIST-BATCH(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-playlist-batch** — Generates signed batches from transaction input

SYNOPSIS
========
**splinter playlist batch** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command generates signed batches from transaction input. The transaction
input is expected to be length-delimited protobuf transaction messages, which
should also be pre-signed for submission to the validator.

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
: The source of input transactions. The transaction input is expected to be
  length-delimited protobuf.

`-k, --key PRIVATE-KEY-FILE`
: Specifies the full path to the private key file. The key will be used to
  sign the batches.

`-n, --max-batch-size NUMBER`
: The maximum number of transactions to include in a batch. (Defaults to 1)

`-o, --output FILE`
: The target for the signed batches.


EXAMPLES
========
The following shows providing a transaction file `txns.txt` and creating
`batches.txt` file of length-delimited protobuf batches.

```
splinter playlist batch \
  --input txns.dat \
  --key ./alice.priv \
  --output batches.dat
```


SEE ALSO
========
| `splinter(1)`
| `splinter-playlist(1)`
| `splinter-playlist-create(1)`
| `splinter-playlist-process(1)`
| `splinter-playlist-submit(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
