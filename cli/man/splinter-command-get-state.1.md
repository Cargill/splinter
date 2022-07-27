% SPLINTER-COMMAND-GET-STATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-command-get-state** — Submits a Sabre transaction to request a state 
read

SYNOPSIS
========
| **splinter command get-state** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command submits a Sabre transaction to request a state read of the
addresses given. Nothing will be displayed after running this command. This
command submits a state read request but does not return the data that is set
for the given address.

This command assumes the distributed ledger's REST API supports Cylinder
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
`-k, --key PRIVATE-KEY-FILE`
: Specifies the full path to the private key file. The key will be used to
  sign the batches as well as generate a JWT for authentication.

`--target TARGET`
: Node URL to submit batches to. The URL should include all of the information
  required to append `/batches` to the end.

`--address ADDRESS`
: State address of the state to be read. This option can be used multiple times
  to specify more than one address to be read.


EXAMPLES
========
The following shows submitting a get state transaction to a Splinter circuit
`vpENT-eSfFZ` with scabbard services. A scabbard service runs a Sabre 
transaction handler. The command smart contract must already be uploaded to
scabbard.

```
splinter command get-state \
  --key /alice.priv \
  --target "http://0.0.0.0:8080/scabbard/vpENT-eSfFZ/gsAA" \
  --address \
  06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583
```

The following shows submitting two get state transactions to a Splinter circuit
`kpHVT-sjpQM` with scabbard services. A scabbard service runs a Sabre 
transaction handler. The command smart contract must already be uploaded to
scabbard.

```
splinter command get-state \
  --key /alice.priv \
  --target "http://0.0.0.0:8080/scabbard/kpHVT-sjpQM/gsAA" \
  --address \
  06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583 \
  --address \
  06abbc6d201beeefb589b08ef0672dac82353d0cbd9ad99e1642c83a1601f3d647bcca
```


SEE ALSO
========
| `splinter(1)`
| `splinter-command(1)`
| `splinter-command-set-state(1)`
| `splinter-command-show-state(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
