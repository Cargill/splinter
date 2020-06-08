% SCABBARD-EXEC(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

NAME
====

**scabbard-exec** — Executes a Sabre smart contract.

SYNOPSIS
========

**scabbard exec** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command executes a smart contract on the targeted scabbard service.
The name and version of the contract must be provided, and they must match a
registered smart contract in the targeted scabbard service. A payload file
provides the data for the smart contract execution, and one or more input/output
addresses must be specified for the transaction.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======
`-C`, `--contract` CONTRACT
: Specifies the smart contract to execute, using the format `NAME:VERSION`. The
  given name and version must exactly match a smart contract that has been
  uploaded and registered in the scabbard service's state. This option is
  required.

`--inputs` ADDRESS
: Specifies an input address for the execution of the smart contract. Inputs are
  state addresses that this transaction is allowed to read from; the smart
  contract must have read permissions for the namespace of each input. This
  option may be provided multiple times to specify multiple input addresses. One
  or more input addresses must be provided.

`-k`, `--key` FILE
: Indicates the key file to use for signing scabbard transactions. The `FILE`
  can be a relative or absolute file path, or it can be the name of a .priv file
  in the `$HOME/.splinter/keys` directory. The target file must contain a valid
  secp256k1 private key. This option is required.

`--outputs` ADDRESS
: Specifies an output address for the execution of the smart contract. Outputs
  are state addresses that this transaction is allowed to write to; the smart
  contract must have write permissions for the namespace of each output. This
  option may be provided multiple times to specify multiple output addresses.
  One or more output addresses must be provided.

`-p`, `--payload` FILE
: Provides the payload in the file to the smart contract for execution. This
  option is required.

`--service-id` ID
: Specifies the fully-qualified service ID of the targeted scabbard service,
  using the format `CIRCUIT_ID::SERVICE_ID`. This option is required.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API that is running the targeted
  scabbard service. (default `http://localhost:8080`) This option is required.

`--wait` SECONDS
: If provided, waits the given number of seconds for the batch to commit.
  Displays an error message if the batch does not commit in time.

EXAMPLES
========
The following command executes version `0.1.0` of the `xo` smart contract in a
scabbard service on circuit `01234-ABCDE` with service ID `abcd`, running on the
node with the REST API endpoint `http://localhost:8088`. The transaction will be
signed with the key located in the file `~/user.priv`. Three addresses are
provided as both inputs and outputs, and the payload file at `~/xo-payload-1`
will be used.

```
$ scabbard exec \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --contract xo:0.1.0 \
  --inputs 00ec03 \
  --inputs cad11d \
  --inputs 5b7349 \
  --outputs 00ec03 \
  --outputs cad11d \
  --outputs 5b7349 \
  --payload ~/xo-payload-1
```

The next command executes version `0.1.2` of the `intkey_multiply` contract in
the same scabbard service, but specifies a key in the `$HOME/.splinter/keys`
directory by name. It also waits up to 10 seconds for the contract execution
batch to commit.

```
$ scabbard exec \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --wait 10 \
  --contract intkey_multiply:0.1.2 \
  --inputs abcdef \
  --outputs abcdef \
  --payload ~/intkey-multiply-payload-1
```

SEE ALSO
========
| Splinter documentation: https://www.splinter.dev/docs/
