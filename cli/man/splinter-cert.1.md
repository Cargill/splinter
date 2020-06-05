% SPLINTER-CERT(1) Cargill, Incorporated | Splinter Commands
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

**splinter-cert** — Provides certificate management subcommands

SYNOPSIS
========

**splinter** **cert** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========

This command provides subcommands for working with self-signed (insecure)
certificates in a development environment. For example, the `splinter cert
generate` subcommand creates these certificates and the associated keys.

Running Splinter in TLS mode usually requires valid X.509 certificates from a
certificate authority. When developing against Splinter, you can use self-signed
development certificates in place of the X.509 certificates. These self-signed
certificates are insecure, so they should only be used in a development
environment (not in a POC or production environment).

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

SUBCOMMANDS
===========

`generate`
: Generates insecure certificates for development

SEE ALSO
========
| `splinter-cert-generate(1)`
| 
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md

