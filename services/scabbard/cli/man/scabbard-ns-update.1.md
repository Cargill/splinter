% SCABBARD-NS-UPDATE(1) Cargill, Incorporated | Splinter Commands
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

**scabbard-ns-update** — Updates the owners of a Sabre namespace.

SYNOPSIS
========

**scabbard ns update** \[**FLAGS**\] \[**OPTIONS**\] NAMESPACE

DESCRIPTION
===========
This command allows users to update the owners of an existing Sabre namespace in
state for the targeted scabbard service. All of the existing owners will be
replaced by the owners provided with this command.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======
`-k`, `--key` FILE
: Indicates the key file to use for signing scabbard transactions. The `FILE`
  can be a relative or absolute file path, or it can be the name of a .priv file
  in the `$HOME/.splinter/keys` directory. The target file must contain a valid
  secp256k1 private key. This option is required.

`-O`, `--owners` KEY
: Includes the given public keys as owners of the namespace. The namespace must
  have one or more owners.

`--service-id` ID
: Specifies the fully-qualified service ID of the targeted scabbard service,
  using the format `CIRCUIT_ID::SERVICE_ID`. This option is required.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API that is running the targeted
  scabbard service. (default `http://localhost:8080`) This option is required.

`--wait` SECONDS
: If provided, waits the given number of seconds for the batch to commit.
  Displays an error message if the batch does not commit in time.

ARGUMENTS
=========
`NAMESPACE`
: Provides the state address prefix of the namespace to update.

EXAMPLES
========
The following command updates the owners of the `abcdef` namespace in a scabbard
service on circuit `01234-ABCDE` with service ID `abcd`, running on the node
with the REST API endpoint `http://localhost:8088`. The namespace will be
updated to have just one owner, and the transaction will be signed with the key
located in the file `~/user.priv`.

```
$ scabbard ns update \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  abcdef
```

The next command updates the owners of the `012345` namespace in the same
scabbard service, but includes multiple owners and specifies a key in the
`$HOME/.splinter/keys` directory by name. It also waits up to 10 seconds for the
namespace update batch to commit.

```
$ scabbard ns update \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  --owner 7b6c889058c2d22558ead2c61b321634b74e705c42f890e6b7bc2c80abb4713118 \
  --owner 02381b606ac2bbe3bd374654cb7cb467ffb0225eb46038a5ec37b43e0c2f085dcb \
  --wait 10 \
  012345
```

SEE ALSO
========
| `scabbard-ns-create(1)`
| `scabbard-ns-delete(1)`
| `scabbard-perm(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
