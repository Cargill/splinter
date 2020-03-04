# Connection Manager Example

This is an example application meant to demonstrate the current functionality
of the connection manager. It is split into two components, a cli tool and a
server. The servier side is a very stripped down version of splinterd as it
can only accept new connections and remove them. The cli tool simply 
dispatches commands to the server.

## Getting Started

Run the following command to set up an environment with two nodes

```
$ docker-compose -f examples/cm-repl/docker-compose.yaml
```

## Using the connectin manager client

The connection manager client (cmc) can add remove and list connections held by
the two nodes.

```
$ docker exec -it cm-repl_node-0_1 bash

# cmc connection add tcp://node-0:3040
# cmc connection list
# cmc connection remove tcp://node-0:3040
```


