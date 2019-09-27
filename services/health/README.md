# Health Service

The Splinter Health Service (SHS) is a Service used to evaluate the integrity
and health of a splinter network.

## Features

### Active Tests

1) Verify peer information in node config matches what is in node state
2) Ping admin services of peers to test peers are alive
3) Create circuits
4) Ping other health services using created circuit
5) Test two phase commit

### Passive Tests

1) Health service sends heartbeats to other health services in circuit

## CLI Commands

### `status`

`status` retrieves a node's peering status.

```bash
splinter health status --url <node_url>
```

### `ping`

`ping` takes a list of peers and pings each of their admin services until a user interrupt
is sent (ctrl-c) or each admin service has been pinged `count` times if the `count` option
was included. If only one node is specified `ping` will ping the peers the node has
stored in state.

```bash
splinter health ping --url <node_url> [<node_url>...] --count <count>
```

### `test create-circuit`

`test create-circuit` creates a circuit between two or more nodes for testing, then
destroys it.

```bash
splinter health test create-circuit --url <node_url> [<node_url>...]
```

### `test connectivity`

`test connectivity` creates a circuit between the listed nodes, then pings each node in the
circuit. The circuit is then destroyed at the end of the test.

```bash
splinter health test connectivity --url <node_url> [<node_url>...]
```

### `test two-phase-commit`

`test two-phase-commit` creates a circuit between the listed nodes and tests that
 two-phase-commit is functioning properly on the created circuit. The circuit
 is then destroyed at the end of the test.

```bash
splinter health test two-phase-commit --url <node_url> [<node_url>...]
```

## REST API

Health Service REST API.

### GET /health/status

Returns basic information about the node's health and version

Response

```json
{
    "version": "0.1",
}
```

### POST /health/ping

Payload

```json
{
    "count": 2,
    "nodes": [
        "tcp://entnet-node0.com",
        "tcp://entnet-node1.com",
        "tcp://entnet-node2.com"
    ]
}
```

Response

```json
{
    "result": "success"
}
```

### POST /health/test/create-circuit

Payload

```json
[
    "tcp://entnet-node0.com",
    "tcp://entnet-node1.com",
    "tcp://entnet-node2.com"
]
```

Response

```json
{
    "result": "success"
}
```

### POST /health/test/connectivity

Payload

```json
[
    "tcp://entnet-node0.com",
    "tcp://entnet-node1.com",
    "tcp://entnet-node2.com"
]
```

Response

```json
{
    "result": "success"
}
```

### POST /health/test/two-phase-commit

Payload

```json
[
    "tcp://entnet-node0.com",
    "tcp://entnet-node1.com",
    "tcp://entnet-node2.com"
]
```

Response

```json
{
    "result": "success"
}
```
