% SCABBARD-CONTRACT-UPLOAD(1) Cargill, Incorporated | Splinter Commands
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

**scabbard-contract-upload** — Uploads a smart contract to scabbard

SYNOPSIS
========

**scabbard contract upload** \[**FLAGS**\] \[**OPTIONS**\] SCAR

DESCRIPTION
===========
This command takes a sabre contract archive (scar) file and uploads its smart
contract to a scabbard service. The scar file is specified using a name, version
requirement, and a list of paths. The file to upload is dynamically determined
by searching the specified paths for a scar file matching the given name and
version requirement. If multiple scar files are found that match the
name/version, the file with the latest version will be used.

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

`-p`, `--path` PATH
: Specifies the directory path(s) to use when searching for the scar file to
  upload. This option can be specified multiple times to provide multiple
  directories to search. If this option is not provided, the `$SCAR_PATH`
  environment variable will be checked. If the environment variable has not been
  set, the default path `/usr/share/scar` will be used.

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
`SCAR`
: Specifies the name and version requirements of the scar file to upload, using
  the format `NAME:VERSION_REQ`. The name must not include underscores (`_`),
  since these are invalid in scar file names. The version requirement can be any
  valid semantic versioning requirement string (for details on semantic
  versioning, visit https://semver.org/).

ENVIRONMENT VARIABLES
=====================
**SCAR_PATH_ENV_VAR**
: List of directories to use when searching for the scar file to upload. (See
  `-p`, `--path`.)

EXAMPLES
========
The following command uploads the smart contract from a scar file located at
`/usr/share/scar/xo_0.3.3.scar`. It uploads the contract to a scabbard service
on circuit `01234-ABCDE` with service ID `abcd`, running on the node with the
REST API endpoint `http://localhost:8088`.

```
$ scabbard contract upload \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  xo:0.3.3
```

For the next command, there are two scar files in the `~/scar` directory:
`intkey_0.1.0.scar` and `intkey_0.1.1.scar`. This command uploads the smart
contract from `intkey_0.1.1.scar`, since it specifies a minimum version
requirement of `0.1`, and `0.1.1` is later than `0.1.0`.

```
$ scabbard contract upload \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --path ~/scar \
  xo:0.1
```

The next example uploads the contract from the same `intkey_0.1.1.scar` file to
he same scabbard service, but it uses a wildcard to match any version. It also
specifies a key in the `$HOME/.splinter/keys` directory by name and waits up to
10 seconds for the contract upload batch to commit.

```
$ scabbard contract upload \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --wait 10 \
  --path ~/scar \
  xo:*
```

SEE ALSO
========
| `scabbard-contract-list(1)`
| `scabbard-contract-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
