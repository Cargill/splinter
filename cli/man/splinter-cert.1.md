% SPLINTER-CERT(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
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
| Splinter documentation: https://www.splinter.dev/docs/0.5/
