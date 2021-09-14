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

//! Contains an influxdb specific implementation of the [metrics::Recorder](https://docs.rs/metrics/0.12.0/metrics/trait.Recorder.html)
//! trait. InfluxRecorder enables using the metrics macros and sending the metrics data to a
//! running Influx database.
//!
//! Available if the `metrics` feature is enabled

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use influxdb::Client;
use influxdb::InfluxDbWriteable;
use metrics::{GaugeValue, Key, Label, Recorder, SharedString, Unit};
use tokio_0_2::runtime::Runtime;
use tokio_0_2::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_0_2::task::JoinHandle;

use crate::error::InternalError;
use crate::threading::lifecycle::ShutdownHandle;

#[derive(InfluxDbWriteable)]
struct Counter<'a> {
    time: DateTime<Utc>,
    key: &'a str,
    value: u64,
}

struct CounterEntry {
    time: DateTime<Utc>,
    value: u64,
}

#[derive(InfluxDbWriteable)]
struct Gauge<'a> {
    time: DateTime<Utc>,
    key: &'a str,
    value: f64,
}

struct GaugeEntry {
    time: DateTime<Utc>,
    value: f64,
}

#[derive(InfluxDbWriteable)]
struct Histogram<'a> {
    time: DateTime<Utc>,
    key: &'a str,
    value: f64,
}

enum MetricRequest {
    Counter {
        key: SharedString,
        value: u64,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Gauge {
        key: SharedString,
        value: GaugeValue,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Histogram {
        key: SharedString,
        value: f64,
        labels: Vec<Label>,
        time: DateTime<Utc>,
    },
    Shutdown,
}

/// Enables using the metrics macros and sending the metrics data to a running Influx database
pub struct InfluxRecorder {
    sender: UnboundedSender<MetricRequest>,
    join_handle: JoinHandle<()>,
    rt: Runtime,
}

impl InfluxRecorder {
    fn new(
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
            let mut counters: HashMap<Box<str>, CounterEntry> = HashMap::new();
            let mut gauges: HashMap<Box<str>, GaugeEntry> = HashMap::new();
            loop {
                match recv.recv().await {
                    Some(MetricRequest::Counter {
                        key,
                        value,
                        labels,
                        time,
                    }) => {
                        let counter = {
                            if let Some(mut counter_entry) = counters.get_mut(&*key) {
                                counter_entry.value += value;
                                counter_entry.time = time;
                                Counter {
                                    key: &*key,
                                    value: counter_entry.value,
                                    time: counter_entry.time,
                                }
                            } else {
                                let counter = Counter {
                                    time,
                                    key: &*key,
                                    value,
                                };
                                // Convert the Cow<'_, str> to a Box<str> to only create a pointer
                                // to the immutable str
                                counters.insert(Box::from(&*key), CounterEntry { value, time });

                                counter
                            }
                        };

                        let mut query = counter.into_query(&*key);
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
                        let gauge = {
                            if let Some(mut gauge_entry) = gauges.get_mut(&*key) {
                                match value {
                                    GaugeValue::Absolute(total) => gauge_entry.value = total,
                                    GaugeValue::Increment(amount) => gauge_entry.value += amount,
                                    GaugeValue::Decrement(amount) => gauge_entry.value -= amount,
                                }
                                gauge_entry.time = time;
                                Gauge {
                                    time: gauge_entry.time,
                                    key: &*key,
                                    value: gauge_entry.value,
                                }
                            } else {
                                let mut gauge_value = 0.0;
                                match value {
                                    GaugeValue::Absolute(total) => gauge_value = total,
                                    GaugeValue::Increment(amount) => gauge_value += amount,
                                    GaugeValue::Decrement(amount) => gauge_value -= amount,
                                }
                                gauges.insert(
                                    Box::from(&*key),
                                    GaugeEntry {
                                        value: gauge_value,
                                        time,
                                    },
                                );

                                Gauge {
                                    time,
                                    key: &*key,
                                    value: gauge_value,
                                }
                            }
                        };
                        let mut query = gauge.into_query(&*key);
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
                            key: &key,
                            value,
                        };
                        let mut query = histogram.into_query(&*key);
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

    /// Initialize metric collection by creating the InfluxRecorder that will connect to the
    /// InfluxDB instance. The record is then added to the metrics library as the recorder which
    /// enables sending the metrics data to the database.
    ///
    /// # Arguments
    ///
    /// * `db_url` - The URL to connect the InfluxDB database for metrics collection
    /// * `db_name` - The name of the InfluxDB database for metrics Collection.
    /// * `username` - The username used for authorization with the InfluxDB.
    /// * `password` - The password used for authorization with the InfluxDB.
    pub fn init(
        db_url: &str,
        db_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), InternalError> {
        let recorder = Self::new(db_url, db_name, username, password)?;
        metrics::set_boxed_recorder(Box::new(recorder))
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
    fn increment_counter(&self, key: &Key, value: u64) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Counter {
            key: name,
            labels,
            value,
            time: Utc::now(),
        }) {
            error!("Unable to to increment counter metric, {}", err);
        };
    }

    fn update_gauge(&self, key: &Key, value: GaugeValue) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Gauge {
            key: name,
            labels,
            value,
            time: Utc::now(),
        }) {
            error!("Unable to update gauge metric, {}", err);
        };
    }

    fn record_histogram(&self, key: &Key, value: f64) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Histogram {
            key: name,
            labels,
            value,
            time: Utc::now(),
        }) {
            error!("Unable to record histogram metric, {}", err);
        };
    }

    fn register_counter(&self, key: &Key, _unit: Option<Unit>, _description: Option<&'static str>) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Counter {
            key: name,
            labels,
            value: 0,
            time: Utc::now(),
        }) {
            error!("Unable to to register counter metric, {}", err);
        };
    }

    fn register_gauge(&self, key: &Key, _unit: Option<Unit>, _description: Option<&'static str>) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Gauge {
            key: name,
            labels,
            value: GaugeValue::Absolute(0.0),
            time: Utc::now(),
        }) {
            error!("Unable to register gauge metric, {}", err);
        };
    }

    fn register_histogram(
        &self,
        key: &Key,
        _unit: Option<Unit>,
        _description: Option<&'static str>,
    ) {
        let (name, labels) = key.clone().into_parts();
        if let Err(err) = self.sender.send(MetricRequest::Histogram {
            key: name,
            labels,
            value: 0.0,
            time: Utc::now(),
        }) {
            error!("Unable to register histogram metric, {}", err);
        };
    }
}
