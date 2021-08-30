// Copyright 2018-2021 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Contains an influxdb specific implementation of the metrics::Recorder trait. InfluxRecorder
//! enables using the metrics macros and sending the metrics data to a running Influx database.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use influxdb::Client;
use influxdb::InfluxDbWriteable;
use metrics_lib::{Key, Label, Recorder};
use tokio_0_2::runtime::Runtime;
use tokio_0_2::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_0_2::task::JoinHandle;

use crate::error::InternalError;
use crate::threading::lifecycle::ShutdownHandle;

#[derive(InfluxDbWriteable, Clone)]
struct Counter {
    time: DateTime<Utc>,
    key: String,
    value: u64,
}

#[derive(InfluxDbWriteable, Clone)]
struct Gauge {
    time: DateTime<Utc>,
    key: String,
    value: i64,
}

#[derive(InfluxDbWriteable, Clone)]
struct Histogram {
    time: DateTime<Utc>,
    key: String,
    value: u64,
}

enum MetricRequest {
    Counter {
        key: String,
        value: u64,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Gauge {
        key: String,
        value: i64,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Histogram {
        key: String,
        value: u64,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Shutdown,
}

pub struct InfluxRecorder {
    sender: UnboundedSender<MetricRequest>,
    join_handle: JoinHandle<()>,
    rt: Runtime,
}

impl InfluxRecorder {
    pub fn new(
        db_url: &str,
        db_name: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, InternalError> {
        let (sender, mut recv) = unbounded_channel();
        let rt = Runtime::new().map_err(|_| {
            InternalError::with_message("Unable to start metrics runtime".to_string())
        })?;

        let client = Client::new(db_url, db_name).with_auth(username, password);

        let join_handle = rt.spawn(async move {
            let mut counters: HashMap<String, Counter> = HashMap::new();
            loop {
                match recv.recv().await {
                    Some(MetricRequest::Counter {
                        key,
                        value,
                        labels,
                        time,
                    }) => {
                        let counter = {
                            if let Some(mut counter) = counters.get_mut(&key) {
                                counter.value += value;
                                counter.time = time;
                                counter.clone()
                            } else {
                                let counter = Counter {
                                    time,
                                    key: key.to_string(),
                                    value,
                                };
                                counters.insert(key.to_string(), counter.clone());
                                counter
                            }
                        };

                        let mut query = counter.into_query(key);
                        for label in labels {
                            query = query.add_tag(label.key(), label.value());
                        }
                        if let Err(err) = client.query(&query).await {
                            error!("Unable to submit influx query: {}", err)
                        };
                    }
                    Some(MetricRequest::Gauge {
                        key,
                        value,
                        labels,
                        time,
                    }) => {
                        let gauge = Gauge {
                            time,
                            key: key.to_string(),
                            value,
                        };
                        let mut query = gauge.into_query(key);
                        for label in labels {
                            query = query.add_tag(label.key(), label.value());
                        }
                        if let Err(err) = client.query(&query).await {
                            error!("Unable to submit influx query: {}", err)
                        };
                    }
                    Some(MetricRequest::Histogram {
                        key,
                        value,
                        labels,
                        time,
                    }) => {
                        let histogram = Histogram {
                            time,
                            key: key.to_string(),
                            value,
                        };
                        let mut query = histogram.into_query(key);
                        for label in labels {
                            query = query.add_tag(label.key(), label.value());
                        }
                        if let Err(err) = client.query(&query).await {
                            error!("Unable to submit influx query: {}", err)
                        };
                    }
                    Some(MetricRequest::Shutdown) => {
                        info!("Received MetricRequest::Shutdown");
                        break;
                    }
                    _ => unimplemented!(),
                }
            }
        });

        Ok(Self {
            sender,
            join_handle,
            rt,
        })
    }

    pub fn init(
        db_url: &str,
        db_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), InternalError> {
        let recorder = Self::new(db_url, db_name, username, password)?;
        metrics_lib::set_boxed_recorder(Box::new(recorder))
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

impl ShutdownHandle for InfluxRecorder {
    fn signal_shutdown(&mut self) {
        if self.sender.send(MetricRequest::Shutdown).is_err() {
            error!("Unable to send shutdown message to InfluxRecorder");
        }
    }

    fn wait_for_shutdown(mut self) -> Result<(), InternalError> {
        self.rt.block_on(self.join_handle).map_err(|err| {
            InternalError::with_message(format!("Unable to join InfluxRecorder thread: {:?}", err))
        })
    }
}

impl Recorder for InfluxRecorder {
    fn increment_counter(&self, key: Key, value: u64) {
        let name = key.name().to_string();
        if let Err(err) = self.sender.send(MetricRequest::Counter {
            key: name,
            labels: key.labels().cloned().collect(),
            value,
            time: Utc::now(),
        }) {
            error!("Unable to to increment counter metric, {}", err);
        };
    }

    fn update_gauge(&self, key: Key, value: i64) {
        let name = key.name().to_string();
        if let Err(err) = self.sender.send(MetricRequest::Gauge {
            key: name,
            labels: key.labels().cloned().collect(),
            value,
            time: Utc::now(),
        }) {
            error!("Unable to update gauge metric, {}", err);
        };
    }

    fn record_histogram(&self, key: Key, value: u64) {
        let name = key.name().to_string();
        if let Err(err) = self.sender.send(MetricRequest::Histogram {
            key: name,
            labels: key.labels().cloned().collect(),
            value,
            time: Utc::now(),
        }) {
            error!("Unable to record histogram metric, {}", err);
        };
    }
}
