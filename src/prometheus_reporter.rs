use crate::metrics::{BenchRunMetrics, ExternalMetricsServiceReporter};
use histogram::Histogram;
use log::info;
use prometheus::core::{AtomicI64, GenericGauge, GenericGaugeVec};
use prometheus::{BasicAuthentication, HistogramOpts, Opts, Registry};
use regex::Regex;
use std::collections::HashMap;
use std::io;

pub struct PrometheusReporter {
    job: String,
    labels: HashMap<String, String>,
    address: String,
    basic_auth: Option<prometheus::BasicAuthentication>,
}

impl ExternalMetricsServiceReporter for PrometheusReporter {
    fn report(&self, metrics: &BenchRunMetrics) -> io::Result<()> {
        info!("Sending metrics to Prometheus: {}", self.address);

        let registry = PrometheusReporter::build_registry(metrics);

        let metric_families = registry.gather();

        prometheus::push_metrics(
            &self.job,
            self.labels.clone(),
            &self.address,
            metric_families,
            match self.basic_auth.as_ref() {
                None => None,
                Some(auth) => Some(BasicAuthentication {
                    username: auth.username.clone(),
                    password: auth.password.clone(),
                }),
            },
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

/// For reporting to Prometheus
impl PrometheusReporter {
    pub fn new(addr: String, job: Option<&str>, labels: Option<&str>) -> Self {
        Self {
            job: job.unwrap_or("pushgateway").to_string(),
            labels: PrometheusReporter::parse_labels(labels),
            address: addr,
            basic_auth: None,
        }
    }

    fn parse_labels(labels: Option<&str>) -> HashMap<String, String> {
        labels
            .map(|value| {
                let regex = Regex::new(r"(?P<key>[a-z][\w\-]+)=(?P<value>[\w\-]+)")
                    .expect("Bad key=value regexp");

                value
                    .split(',')
                    .map(|item| {
                        let mut key = None;
                        let mut value = None;

                        for k in regex.captures_iter(item) {
                            if let Some(m) = k.name("key") {
                                key = Some(m.as_str());
                            }
                            if let Some(m) = k.name("value") {
                                value = Some(m.as_str());
                            }
                        }
                        (
                            key.expect("Key is not present").to_string(),
                            value.expect("Value is not present").to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or(HashMap::new())
    }

    fn build_registry(bench_run_metrics: &BenchRunMetrics) -> Registry {
        let registry = Registry::new();

        PrometheusReporter::register_gauge(
            &registry,
            "request_count",
            "All requests",
            bench_run_metrics.total_requests as i64,
        );
        PrometheusReporter::register_gauge(
            &registry,
            "success_count",
            "Successful requests",
            bench_run_metrics.successful_requests as i64,
        );
        PrometheusReporter::register_gauge(
            &registry,
            "bytes_count",
            "Bytes received/sent",
            bench_run_metrics.total_bytes as i64,
        );

        PrometheusReporter::register_codes(
            &registry,
            "response_codes",
            "Response codes/errors",
            &bench_run_metrics.summary,
        );
        PrometheusReporter::register_histogram(
            &registry,
            "success_latency",
            "Latency of successful requests",
            bench_run_metrics.success_latency.clone(),
        );

        PrometheusReporter::register_histogram(
            &registry,
            "error_latency",
            "Latency of failed requests",
            bench_run_metrics.error_latency.clone(),
        );

        let mut latency = bench_run_metrics.success_latency.clone();
        latency.merge(&bench_run_metrics.error_latency);
        PrometheusReporter::register_histogram(
            &registry,
            "latency",
            "Latency of failed requests",
            latency,
        );

        registry
    }

    fn register_gauge(registry: &Registry, name: &str, help: &str, value: i64) {
        let gauge = GenericGauge::<AtomicI64>::new(name, help).expect("Creating gauge failed");
        registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| e.to_string())
            .expect("Cannot register gauge");
        gauge.set(value);
    }

    fn register_codes<I: Into<i64> + Copy>(
        registry: &Registry,
        name: &str,
        help: &str,
        map_of_codes: &HashMap<String, I>,
    ) {
        let codes = GenericGaugeVec::<AtomicI64>::new(Opts::new(name, help), &["Code"])
            .expect("Codes failed");
        registry
            .register(Box::new(codes.clone()))
            .map_err(|e| e.to_string())
            .expect("Cannot register codes");

        map_of_codes
            .iter()
            .for_each(|(k, v)| codes.with_label_values(&[&k]).set((*v).into()))
    }

    fn register_histogram(registry: &Registry, name: &str, help: &str, histogram: Histogram) {
        let mut buckets = vec![];
        let mut counts = vec![];
        for bucket in histogram.into_iter() {
            if bucket.count() > 0 {
                buckets.push(bucket.value() as f64);
                counts.push(bucket.count());
            }
        }
        let prometheus_histogram = prometheus::Histogram::with_opts(
            HistogramOpts::new(name, help).buckets(buckets.clone()),
        )
        .expect("Histogram failed");

        registry
            .register(Box::new(prometheus_histogram.clone()))
            .map_err(|e| e.to_string())
            .expect("Cannot register histogram");

        for i in 0..buckets.len() {
            for _ in 0..counts[i] {
                prometheus_histogram.observe(buckets[i] as f64);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::prometheus_reporter::PrometheusReporter;
    use histogram::Histogram;
    use prometheus::proto::*;
    use prometheus::Registry;
    use std::collections::HashMap;

    #[test]
    fn test_parse_labels() {
        let test_cases = vec![
            ("type=plain_nginx", vec![("type", "plain_nginx")]),
            (
                "type=tunneled_nginx,size=10kb",
                vec![("type", "tunneled_nginx"), ("size", "10kb")],
            ),
        ];
        for (str, map) in test_cases {
            let labels = PrometheusReporter::parse_labels(Some(str));
            assert_eq!(labels.len(), map.len());
            for (k, v) in map.into_iter() {
                assert_eq!(labels.get(k), Some(&v.to_string()));
            }
        }
    }

    #[test]
    fn test_register_codes() {
        let registry = Registry::new();
        let mut counters = HashMap::new();
        counters.insert("200".to_string(), 100);
        counters.insert("500".to_string(), 1);
        PrometheusReporter::register_codes(
            &registry,
            "response_codes",
            "HTTP response codes",
            &counters,
        );
        let metrics = registry.gather();
        assert_eq!(1, metrics.len());
        assert_eq!("response_codes", metrics[0].get_name());
        assert_eq!("HTTP response codes", metrics[0].get_help());
        assert_eq!(MetricType::GAUGE, metrics[0].get_field_type());

        assert_eq!("Code", metrics[0].get_metric()[0].get_label()[0].get_name());
        assert_eq!("200", metrics[0].get_metric()[0].get_label()[0].get_value());
        assert_eq!(100., metrics[0].get_metric()[0].get_gauge().get_value());

        assert_eq!("Code", metrics[0].get_metric()[1].get_label()[0].get_name());
        assert_eq!("500", metrics[0].get_metric()[1].get_label()[0].get_value());
        assert_eq!(1., metrics[0].get_metric()[1].get_gauge().get_value());
    }

    #[test]
    fn test_register_histogram() {
        let registry = Registry::new();
        let mut histogram = Histogram::new();
        histogram.increment(100).expect("infallible");
        histogram.increment(200).expect("infallible");
        histogram.increment(300).expect("infallible");
        histogram.increment(300).expect("infallible");

        PrometheusReporter::register_histogram(
            &registry,
            "latency",
            "Latency of requests",
            histogram,
        );

        let metrics = registry.gather();

        assert_eq!(1, metrics.len());
        assert_eq!("latency", metrics[0].get_name());
        assert_eq!("Latency of requests", metrics[0].get_help());
        assert_eq!(MetricType::HISTOGRAM, metrics[0].get_field_type());

        let prometheus_histogram = metrics[0].get_metric()[0].get_histogram();
        let buckets = prometheus_histogram.get_bucket();
        assert_eq!(3, buckets.len());
        assert_eq!(100., buckets[0].get_upper_bound());
        assert_eq!(1, buckets[0].get_cumulative_count());
        assert_eq!(200., buckets[1].get_upper_bound());
        assert_eq!(2, buckets[1].get_cumulative_count());
        assert_eq!(300., buckets[2].get_upper_bound());
        assert_eq!(4, buckets[2].get_cumulative_count());
    }
}
