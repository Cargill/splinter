% TRANSACT-COMMAND-SHOW-STATE(1) Cargill, Incorporated | Transact Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**transact-command-show-state** — Displays the state value at a given address

SYNOPSIS
========
| **transact command show state** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Display the state value at the given state address if it exists.

This command assumes the distributed ledger's REST API supports Cylinder
JWT authentication.

FLAGS
=====
`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-t`, `--text`
: Attempt to convert the state value from bytes and display it as an ascii
  string.

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
: Node URL to retrieve the state value from.

`--address ADDRESS`
: State address of the state value to be retrieved.


EXAMPLES
========
The following shows retrieving the state value at the address 
`06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583`
from a Splinter circuit `vpENT-eSfFZ` with scabbard services. The command smart
contract must already be uploaded to scabbard.

```
transact command show-state \
  --key /alice.priv \
  --target "http://0.0.0.0:8080/scabbard/vpENT-eSfFZ/gsAA" \
  --address \
  06abbcb16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a1583
```


SEE ALSO
========
| `transact(1)`
| `transact-command(1)`
| `transact-command-set-state(1)`
| `transact-command-get-state(1)`
|
| Transact documentation: https://docs.rs/transact/latest
