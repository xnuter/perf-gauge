use crate::metrics::{BenchRunMetrics, ExternalMetricsServiceReporter};
use histogram::Histogram;
use log::info;
use prometheus::core::{AtomicI64, GenericGauge, GenericGaugeVec};
use prometheus::{BasicAuthentication, HistogramOpts, Opts, Registry};
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
    pub fn new(addr: String, job: Option<&str>, labels: Vec<(String, String)>) -> Self {
        Self {
            job: job.unwrap_or("pushgateway").to_string(),
            labels: labels.into_iter().collect(),
            address: addr,
            basic_auth: None,
        }
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
    use crate::metrics::{
        BenchRunMetrics, DefaultConsoleReporter, ExternalMetricsServiceReporter, RequestStats,
        RequestStatsBuilder,
    };
    use crate::prometheus_reporter::PrometheusReporter;
    use histogram::Histogram;
    use mockito::mock;
    use prometheus::proto::*;
    use prometheus::Registry;
    use std::collections::HashMap;
    use std::time::Duration;

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

    #[test]
    fn test_build_registry() {
        let mut metrics = BenchRunMetrics::new();
        let mut total_bytes = 0;
        let mut successful_requests = 0;
        let mut total_requests = 0;

        for i in 1..=100 {
            let (success, code) = if i % 5 == 0 {
                (true, "200".to_string())
            } else {
                (false, "500".to_string())
            };
            total_bytes += i;
            successful_requests += if success { 1 } else { 0 };
            total_requests += 1;

            metrics.report_request(
                RequestStatsBuilder::default()
                    .bytes_processed(i)
                    .status(code)
                    .is_success(success)
                    .duration(Duration::from_micros(i as u64))
                    .build()
                    .expect("RequestStatsBuilder failed"),
            );
        }
        DefaultConsoleReporter::new()
            .report(&metrics)
            .expect("infallible");

        let registry = PrometheusReporter::build_registry(&metrics);

        let metrics = registry.gather();

        let mut metrics_map = HashMap::new();

        for m in metrics.iter() {
            metrics_map.insert(m.get_name(), m);
        }

        let bytes_count = metrics_map.get("bytes_count").expect("Missing bytes_count");
        let error_latency = metrics_map
            .get("error_latency")
            .expect("Missing error_latency");
        let latency = metrics_map.get("latency").expect("Missing latency");
        let request_count = metrics_map
            .get("request_count")
            .expect("Missing request_count");
        let response_codes = metrics_map
            .get("response_codes")
            .expect("Missing response_codes");
        let success_count = metrics_map
            .get("success_count")
            .expect("Missing success_count");
        let success_latency = metrics_map
            .get("success_latency")
            .expect("Missing success_latency");

        assert_eq!(MetricType::GAUGE, bytes_count.get_field_type());
        assert_eq!(MetricType::GAUGE, request_count.get_field_type());
        assert_eq!(MetricType::GAUGE, success_count.get_field_type());
        assert_eq!(MetricType::GAUGE, response_codes.get_field_type());
        assert_eq!(MetricType::HISTOGRAM, latency.get_field_type());
        assert_eq!(MetricType::HISTOGRAM, success_latency.get_field_type());
        assert_eq!(MetricType::HISTOGRAM, error_latency.get_field_type());

        assert_eq!(
            total_bytes as f64,
            bytes_count.get_metric()[0].get_gauge().get_value()
        );
        assert_eq!(
            total_requests as f64,
            request_count.get_metric()[0].get_gauge().get_value()
        );
        assert_eq!(
            successful_requests as f64,
            success_count.get_metric()[0].get_gauge().get_value()
        );

        assert_eq!(
            "Code",
            response_codes.get_metric()[0].get_label()[0].get_name()
        );
        assert_eq!(
            "200",
            response_codes.get_metric()[0].get_label()[0].get_value()
        );
        assert_eq!(
            successful_requests as f64,
            response_codes.get_metric()[0].get_gauge().get_value()
        );

        assert_eq!(
            "Code",
            response_codes.get_metric()[1].get_label()[0].get_name()
        );
        assert_eq!(
            "500",
            response_codes.get_metric()[1].get_label()[0].get_value()
        );
        assert_eq!(
            (total_requests - successful_requests) as f64,
            response_codes.get_metric()[1].get_gauge().get_value()
        );
    }

    #[test]
    fn test_prometheus_reporting() {
        let _m = mock("PUT", "/metrics/job/prometheus_job/k1/v1")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("world")
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);

        let reporter = PrometheusReporter::new(
            url["http://".len()..].to_string(),
            Some("prometheus_job"),
            vec![("k1".to_string(), "v1".to_string())],
        );

        let mut metrics = BenchRunMetrics::new();
        for i in 0..1000 {
            metrics.report_request(RequestStats {
                is_success: true,
                bytes_processed: 0,
                status: "200 OK".to_string(),
                duration: Duration::from_micros(i),
            });
        }

        let sent = reporter.report(&metrics);

        assert!(sent.is_ok(), "{:?}", sent);
    }
}
