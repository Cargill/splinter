% SCABBARD-CONTRACT-SHOW(1) Cargill, Incorporated | Splinter Commands
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

**scabbard-contract-show** — Displays the details of a scabbard smart contract

SYNOPSIS
========

**scabbard contract show** \[**FLAGS**\] \[**OPTIONS**\] CONTRACT

DESCRIPTION
===========
This command shows the details of a specific smart contract that has been
uploaded to a scabbard service.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======
`--service-id` ID
: Specifies the fully-qualified service ID of the targeted scabbard service,
  using the format `CIRCUIT_ID::SERVICE_ID`. This option is required.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API that is running the targeted
  scabbard service. (default `http://localhost:8080`) This option is required.

ARGUMENTS
=========
`CONTRACT`
: Specifies the contract to display, using the format `NAME:VERSION`. The name
  and version must exactly match the name and version of the smart contract.

EXAMPLES
========
The following command displays the details of the `0.3.3` version of the smart
contract named `xo`. This smart contract has been uploaded to the scabbard
service on circuit `01234-ABCDE` with service ID `abcd`, which is running on the
node with the REST API endpoint `http://localhost:8088`.

```
$ scabbard contract show \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  xo:0.3.3
xo 0.3.3
  inputs:
  - 5b7349
  outputs:
  - 5b7349
  creator: 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a
```

SEE ALSO
========
| `scabbard-contract-list(1)`
| `scabbard-contract-upload(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
