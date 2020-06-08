% SPLINTER-CERT-GENERATE(1) Cargill, Incorporated | Splinter Commands
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

**splinter-cert-generate** — Generates test certificates and keys for running
  splinterd with TLS (in insecure mode)

SYNOPSIS
========
| **splinter cert generate** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Running Splinter in TLS mode requires valid X.509 certificates from a
certificate authority. When developing against Splinter, you can use this
command to generate development certificates and the associated keys for your
development environment.

The files are generated in the location specified by `--cert-dir`, the
`SPLINTER_CERT_DIR` environment variable, or in the default location
`/etc/splinter/certs/`. Note: The default location could be different if the
`SPLINTER_HOME` environment variable is set; see the `splinterd(1)` man page
for more information.

The following files are created: `client.crt`, `client.key`, `server.crt`,
`server.key`, `generated_ca.pem`, and `generated_ca.key`.

FLAGS
=====
`--force`
: Overwrites files if they exist. If this flag is not provided and the file
  exists, an error is returned.

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decreases verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`--skip`
: Checks if the files exists and generates the files that are missing. If this
flag is not provided and the file exists, an error is returned.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======
`-d`, `--cert-dir CERT-DIR`
: Specifies the path to the directory to contain the certificates and associated
  key files. (Default: `/etc/splinter/certs/`, unless `SPLINTER_CERT_DIR` or
  `SPLINTER_HOME` is set). This directory must exist.

`--common-name COMMON-NAME`
: Specifies a common name for the generated certificate. (Default: `localhost`.)
  Use this option if the `splinterd` URL uses a DNS address instead of a
  numerical IP address.

EXAMPLES
========
To generate test certificates and keys:

  `$ splinter cert generate`

To create missing certificates and keys when some files already exist, add the
`--skip` flag. The command will ignore the existing files and create any files
that are missing.

  `$ splinter cert generate --skip`

To recreate the certificates and keys from scratch, use the `--force` flag to
overwrite all existing files.

  `$ splinter cert generate --force`

ENVIRONMENT VARIABLES
=====================

**SPLINTER_CERT_DIR**

: Specifies the directory containing certificates and associated key files
  (see `--cert-dir`).

**SPLINTER_HOME**

: Changes the base directory path for the Splinter directories, including the
  certificate directory. (See the `splinterd(1)` man page for more information.)
  This value is not used if `SPLINTER_CERT_DIR` is set.

SEE ALSO
========
| `splinterd(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
