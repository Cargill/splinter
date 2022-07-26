% SPLINTER-COMMAND-SET-STATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-command-set-state** — Submits a Sabre transaction to request a state 
write

SYNOPSIS
========
| **splinter command set-state** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command submits a Sabre transaction to request one or more state write of
the state entries given. The state entry is a key value pair where the key is a
state address and the value is the value to be set for the given address.

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

`--state-entry STATE-ENTRY`
: Key-value pair where the key is a state address and the value is the value to
  be set for that address. (format: address:value)


EXAMPLES
========
The following shows submitting a set state transaction to a Splinter circuit
`vpENT-eSfFZ` with scabbard services. A scabbard service runs a Sabre 
transaction handler. The command smart contract must already be uploaded to
scabbard.

```
splinter command set-state \
  --key /alice.priv
  --target "http://0.0.0.0:8080/scabbard/vpENT-eSfFZ/gsAA"
  --state-entry \
  06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583:value
```

The following shows submitting two set state transactions to a Splinter circuit
`kpHVT-sjpQM` with scabbard services. A scabbard service runs a Sabre 
transaction handler. The command smart contract must already be uploaded to
scabbard.

```
splinter command set-state \
  --key /alice.priv \
  --target "http://0.0.0.0:8080/scabbard/kpHVT-sjpQM/gsAA" \
  --state-entry \
  06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583:value1 \
  --state-entry \
  06abbc6d201beeefb589b08ef0672dac82353d0cbd9ad99e1642c83a1601f3d647bcca:value2
```


SEE ALSO
========
| `splinter(1)`
| `splinter-command(1)`
| `splinter-command-get-state(1)`
| `splinter-command-show-state(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
