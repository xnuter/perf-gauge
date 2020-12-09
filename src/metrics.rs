use bytesize::ByteSize;
use core::fmt;
use histogram::Histogram;
use log::{error, info};
use std::collections::HashMap;
use std::ops::AddAssign;
use std::time::{Duration, Instant};
use std::{cmp, io};

pub trait ExternalMetricsServiceReporter {
    fn report(&self, metrics: &BenchRunMetrics) -> io::Result<()>;
    fn shutdown(&self);
}

pub struct DefaultConsoleReporter {
    test_case_name: Option<String>,
}

#[derive(Clone)]
pub struct BenchRunMetrics {
    pub(crate) bench_begin: Instant,
    pub(crate) total_bytes: usize,
    pub(crate) total_requests: usize,
    pub(crate) successful_requests: usize,
    pub(crate) summary: HashMap<String, i32>,
    pub(crate) success_latency: Histogram,
    pub(crate) error_latency: Histogram,
}

/// Default reporter that prints stats to console.
#[derive(Serialize)]
struct BenchRunReport {
    test_case_name: Option<String>,
    duration: Duration,
    total_bytes: usize,
    total_requests: usize,
    success_rate: f64,
    rate_per_second: f64,
    bitrate_mbps: f64,
    response_code_summary: Vec<(String, i32)>,
    latency_summary: Vec<(String, u64)>,
}

#[derive(Builder, Debug)]
pub struct RequestStats {
    pub is_success: bool,
    pub bytes_processed: usize,
    pub status: String,
    pub duration: Duration,
}

impl BenchRunMetrics {
    pub fn new() -> Self {
        Self {
            bench_begin: Instant::now(),
            total_bytes: 0,
            total_requests: 0,
            successful_requests: 0,
            summary: Default::default(),
            success_latency: Default::default(),
            error_latency: Default::default(),
        }
    }

    pub fn report_request(&mut self, stats: RequestStats) {
        self.total_requests += 1;
        if stats.is_success {
            self.successful_requests += 1;
            self.success_latency
                .increment(stats.duration.as_micros() as u64)
                .unwrap_or_default();
        } else {
            self.error_latency
                .increment(stats.duration.as_micros() as u64)
                .unwrap_or_default();
        }
        self.total_bytes += stats.bytes_processed;
        self.summary.entry(stats.status).or_insert(0).add_assign(1);
    }

    pub fn truncated_mean(&self, threshold: f64) -> u64 {
        let histogram = &self.success_latency;
        let lowest = histogram.percentile(threshold).unwrap_or_default() as i64;
        let highest = histogram.percentile(100. - threshold).unwrap_or_default() as i64;
        let mut ignored_count = 0;
        let mut count = 0;
        let mut sum = 0_u64;
        for bucket in histogram.into_iter() {
            if bucket.value() as i64 >= lowest && bucket.value() as i64 <= highest {
                count += bucket.count();
                sum += bucket.value() * bucket.count();
            } else {
                ignored_count += bucket.count();
            }
        }
        if count > 0 {
            let truncated_mean = sum / count;
            info!(
                "Truncated mean {:.3}: ignored {} data points out of {}, the %={:.6}. TM={}µs",
                threshold,
                ignored_count,
                count + ignored_count,
                ignored_count as f64 * 100. / count as f64,
                truncated_mean
            );
            truncated_mean
        } else {
            error!("No data points");
            0
        }
    }
}

impl BenchRunReport {
    fn summary_ordered(metrics: &BenchRunMetrics) -> Vec<(String, i32)> {
        let mut pairs: Vec<(String, i32)> = metrics
            .summary
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        pairs.sort_by(|a, b| {
            let d = b.1 - a.1;
            match d {
                1..=0x7fffffff => cmp::Ordering::Greater,
                0 => a.0.cmp(&b.0),
                _ => cmp::Ordering::Less,
            }
        });

        pairs
    }

    fn latency_summary(metrics: &BenchRunMetrics) -> Vec<(String, u64)> {
        // for simplicity of reporting we merge both latency
        // into a single histogram.
        let mut latency = metrics.success_latency.clone();
        latency.merge(&metrics.error_latency);

        vec![
            ("Min".to_string(), latency.minimum().unwrap_or_default()),
            (
                "p50".to_string(),
                latency.percentile(50.0).unwrap_or_default(),
            ),
            (
                "p90".to_string(),
                latency.percentile(90.0).unwrap_or_default(),
            ),
            (
                "p99".to_string(),
                latency.percentile(99.0).unwrap_or_default(),
            ),
            (
                "p99.9".to_string(),
                latency.percentile(99.9).unwrap_or_default(),
            ),
            (
                "p99.99".to_string(),
                latency.percentile(99.99).unwrap_or_default(),
            ),
            ("Max".to_string(), latency.maximum().unwrap_or_default()),
            ("Mean".to_string(), latency.mean().unwrap_or_default()),
            ("StdDev".to_string(), latency.stddev().unwrap_or_default()),
            ("tm95".to_string(), metrics.truncated_mean(5.0)),
            ("tm99".to_string(), metrics.truncated_mean(1.0)),
            ("tm99.9".to_string(), metrics.truncated_mean(0.1)),
        ]
    }
}

impl fmt::Display for BenchRunReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.test_case_name.as_ref() {
            None => String::new(),
            Some(value) => format!("Test: {}\n", value),
        };
        writeln!(
            f,
            "{}Duration {:?} \n\
            Requests: {} \n\
            Request rate: {:.3} per second\n\
            Success rate: {:.3}%\n\
            Total bytes: {} \n\
            Bitrate: {:.3} Mbps",
            name,
            self.duration,
            self.total_requests,
            self.rate_per_second,
            self.success_rate,
            ByteSize::b(self.total_bytes as u64).to_string(),
            self.bitrate_mbps,
        )?;

        if !self.response_code_summary.is_empty() {
            writeln!(f)?;

            writeln!(f, "Summary:")?;
            for pair in &self.response_code_summary {
                writeln!(f, "{}: {}", pair.0, pair.1)?;
            }
        }

        if !self.latency_summary.is_empty() {
            writeln!(f)?;
            writeln!(f, "Latency:")?;
            let mut max_label_len = 0;
            let mut max_value_len = 0;
            for (label, value) in self.latency_summary.iter() {
                max_label_len = max_label_len.max(label.len());
                max_value_len = max_value_len.max(value.to_string().len());
            }
            for (label, value) in self.latency_summary.iter() {
                let label_spacing = " ".repeat(max_label_len - label.len() + 1);
                let value_spacing = " ".repeat(max_value_len - value.to_string().len() + 1);
                writeln!(
                    f,
                    "{label}{label_spacing}:{value_spacing}{value}µs",
                    label = label,
                    value = value,
                    label_spacing = label_spacing,
                    value_spacing = value_spacing
                )?;
            }
            Ok(())
        } else {
            writeln!(f)
        }
    }
}

// cov:begin-ignore-line
impl ExternalMetricsServiceReporter for DefaultConsoleReporter {
    fn report(&self, metrics: &BenchRunMetrics) -> io::Result<()> {
        let report = self.build_report(metrics);
        println!("{}", report);
        println!("{}", "=".repeat(50));
        info!(target: "stats", "{}",
              serde_json::to_string(&report).expect("JSON serialization failed"));
        Ok(())
    }

    fn shutdown(&self) {
        // do nothing
    }
}

impl DefaultConsoleReporter {
    pub fn new(test_case_name: Option<String>) -> Self {
        Self { test_case_name }
    }

    fn build_report(&self, metrics: &BenchRunMetrics) -> BenchRunReport {
        let successful_requests = metrics.successful_requests as usize;
        let total_requests = metrics.total_requests as usize;
        let total_bytes = metrics.total_bytes as usize;
        let duration = Instant::now().duration_since(metrics.bench_begin);
        BenchRunReport {
            test_case_name: self
                .test_case_name
                .as_ref()
                .cloned()
                .or_else(|| Some("perf-gauge".to_string())),
            duration,
            total_bytes,
            total_requests,
            success_rate: successful_requests as f64 * 100. / total_requests as f64,
            rate_per_second: total_requests as f64 / duration.as_secs_f64(),
            bitrate_mbps: total_bytes as f64 / duration.as_secs_f64() * 8. / 1_000_000.,
            response_code_summary: BenchRunReport::summary_ordered(&metrics),
            latency_summary: BenchRunReport::latency_summary(&metrics),
        }
    }
}
// cov:end-ignore-line

#[cfg(test)]
mod tests {
    use crate::bench_run::BenchRun;
    use crate::metrics::{BenchRunMetrics, DefaultConsoleReporter, RequestStats};
    use crate::rate_limiter::RateLimiter;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_codes() {
        let mut metrics = BenchRunMetrics::new();
        let codes = vec![
            "200 OK".to_string(),
            "200 OK".to_string(),
            "400 BAD_REQUEST".to_string(),
            "502 BAD_GATEWAY".to_string(),
            "502 BAD_GATEWAY".to_string(),
            "502 BAD_GATEWAY".to_string(),
        ];

        for code in codes.into_iter() {
            metrics.report_request(RequestStats {
                is_success: true,
                bytes_processed: 0,
                status: code,
                duration: Default::default(),
            });
        }

        let mut ordered_summary = DefaultConsoleReporter::new(None)
            .build_report(&metrics)
            .response_code_summary
            .into_iter();
        assert_eq!(
            Some(("502 BAD_GATEWAY".to_string(), 3)),
            ordered_summary.next()
        );
        assert_eq!(Some(("200 OK".to_string(), 2)), ordered_summary.next());
        assert_eq!(
            Some(("400 BAD_REQUEST".to_string(), 1)),
            ordered_summary.next()
        );
    }

    #[test]
    fn test_latency() {
        let mut metrics = BenchRunMetrics::new();
        for i in 0..1000 {
            metrics.report_request(RequestStats {
                is_success: true,
                bytes_processed: 0,
                status: "200 OK".to_string(),
                duration: Duration::from_micros(i),
            });
        }

        let report = DefaultConsoleReporter::new(None).build_report(&metrics);
        let mut items = report.latency_summary.into_iter();

        assert_eq!(Some(("Min".to_string(), 0)), items.next());
        assert_eq!(Some(("p50".to_string(), 500)), items.next());
        assert_eq!(Some(("p90".to_string(), 900)), items.next());
        assert_eq!(Some(("p99".to_string(), 990)), items.next());
        assert_eq!(Some(("p99.9".to_string(), 999)), items.next());
        assert_eq!(Some(("p99.99".to_string(), 999)), items.next());
        assert_eq!(Some(("Max".to_string(), 999)), items.next());
        assert_eq!(Some(("Mean".to_string(), 500)), items.next());
        assert_eq!(Some(("StdDev".to_string(), 289)), items.next());
    }

    #[test]
    fn test_has_more_work_request_limit() {
        let requests = 10;
        let mut metrics =
            BenchRun::with_request_limit(0, requests, RateLimiter::build_rate_limiter(0.));
        for _ in 0..requests {
            assert!(metrics.has_more_work());
        }
        assert!(!metrics.has_more_work());
    }

    #[test]
    fn test_has_more_work_time_limit() {
        let duration = Duration::from_secs(1);
        let mut metrics =
            BenchRun::with_duration_limit(0, duration, RateLimiter::build_rate_limiter(0.));
        for _ in 0..1000 {
            assert!(metrics.has_more_work());
        }
        sleep(duration);
        assert!(!metrics.has_more_work());
    }

    #[test]
    fn test_bench_run_report_display() {
        let mut metrics = BenchRunMetrics::new();
        for i in 0..1000 {
            metrics.report_request(RequestStats {
                is_success: true,
                bytes_processed: 0,
                status: "200 OK".to_string(),
                duration: Duration::from_micros(i),
            });
        }

        let report = DefaultConsoleReporter::new(None).build_report(&metrics);
        let as_str = report.to_string();

        assert!(as_str.contains("Min"));
        assert!(as_str.contains("p50"));
        assert!(as_str.contains("p90"));
        assert!(as_str.contains("p99"));
        assert!(as_str.contains("p99.9"));
        assert!(as_str.contains("p99.99"));
        assert!(as_str.contains("Max"));
        assert!(as_str.contains("Mean"));
        assert!(as_str.contains("StdDev"));
        assert!(as_str.contains("tm95"));
        assert!(as_str.contains("tm99"));
        assert!(as_str.contains("tm99.9"));
    }
}
