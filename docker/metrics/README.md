<!--
  Copyright 2018-2022 Cargill Incorporated

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

Running `just metrics` from the root of the repo will start:

    Grafana server
    Influx DB
    Telegraf

Once all the services have started, open
[http://localhost:3000](http://localhost:3000) in a browser to view Grafana.
The default username and password are both `admin`.

The Influx server is available within docker at `http://splinter-influx:8086`
and exposed on your local machine at `http://localhost:8086`.

A test container named `splinter-telegraf` is provided to feed data into
Grafana to show that everything is working.

Press `^c` to shut down the network.

The compose file makes use of Docker named volumes for grafana and influx.
To delete these volumes,
run `docker-compose -f docker/metrics/docker-compose.yaml down -v` or
`just clean-metrics` from the root of the repo.
