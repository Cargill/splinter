<img alt="Splinter Logo" src="assets/splinter_logos_fulllogo_gradientblack.svg" width="500">

# Splinter

Splinter is a privacy-focused platform for distributed applications that
provides a blockchain-inspired networking environment for communication and
transactions between organizations. Splinter lets you combine blockchain-related
technologies -- such as smart contracts and consensus engines -- to build a wide
variety of architectural patterns.

See [splinter.dev](https://www.splinter.dev/) to learn about Splinter.

## Useful Links

* [Splinter documentation](https://www.splinter.dev/docs/)
* [Release notes](https://www.splinter.dev/releases/)
* [Community information](https://www.splinter.dev/community/)
* [Other Splinter repositories](https://www.splinter.dev/community/repositories.html)
* [Example applications](https://www.splinter.dev/examples/)
* Related projects:
    - [Hyperledger Grid](https://github.com/hyperledger/grid/)
    - [Hyperledger Transact](https://github.com/hyperledger/transact/)
    - [Sawtooth Sabre](https://github.com/hyperledger/sawtooth-sabre/)

## Building Splinter

To build Splinter, run `cargo build` from the root directory. This command
builds all of the Splinter components, including `libsplinter` (the main
library), `splinterd` (the splinter daemon), the CLI, the client, and all
examples in the `examples` directory.

To build individual components, run `cargo build` in the component directories.
For example, to build only the splinter library, navigate to
`libsplinter`, then run `cargo build`.

To build Splinter using Docker, run
`docker-compose -f docker-compose-installed.yaml build` from the root
directory. This command builds Docker images for all of the Splinter
components, including `libsplinter` (the main library), `splinterd`
(the splinter daemon), the CLI, the client, and all examples in the `examples`
directory.

To build individual components using Docker, run
`docker-compose -f docker-compose-installed.yaml build <component>`
from the root directory. For example, to build only the splinter daemon,
run `docker-compose -f docker-compose-installed.yaml build splinterd`.

To use Docker to build Splinter with experimental features enabled, set an
environment variable in your shell before running the build commands. For
example: `export 'CARGO_ARGS=-- --features experimental'`. To go back to
building with default features, unset the environment variable:
`unset CARGO_ARGS`

## License

Splinter software is licensed under the [Apache License Version 2.0](LICENSE)
software license.

## Code of Conduct

Splinter operates under the [Cargill Code of
Conduct](https://github.com/Cargill/code-of-conduct/blob/master/code-of-conduct.md).
