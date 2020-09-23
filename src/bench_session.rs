/// Copyright 2020 Developers of the service-benchmark project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use async_trait::async_trait;
use core::{cmp, fmt};
use histogram::Histogram;
use std::collections::HashMap;
use std::ops::AddAssign;
use std::time::Instant;

pub struct BenchRun {
    pub bench_begin: Instant,
    pub total_bytes: usize,
    pub total_requests: usize,
    summary: HashMap<String, i32>,
    latencies: Histogram,
}

#[derive(Builder, Debug)]
pub struct RequestStats {
    pub bytes_processed: usize,
    pub status: String,
}

#[async_trait]
pub trait BenchClient {
    type Client;

    fn build_client(&self) -> Result<Self::Client, String>;
    async fn send_request(&self, client: &Self::Client) -> Result<RequestStats, String>;
}

impl BenchRun {
    pub fn new() -> Self {
        Self {
            bench_begin: Instant::now(),
            summary: HashMap::new(),
            latencies: Histogram::new(),
            total_bytes: 0,
            total_requests: 0,
        }
    }

    pub fn increment(&mut self, key: String) {
        self.summary.entry(key).or_insert(0).add_assign(1);
    }

    pub fn report_latency(&mut self, elapsed_us: u64) -> Result<(), &str> {
        self.latencies.increment(elapsed_us)
    }

    pub fn merge(&mut self, other: &Self) {
        self.bench_begin = self.bench_begin.min(other.bench_begin);
        self.latencies.merge(&other.latencies);
        self.total_requests += other.total_requests;
        self.total_bytes += other.total_bytes;
        for (k, v) in other.summary.iter() {
            self.summary.entry(k.clone()).or_insert(0).add_assign(v);
        }
    }
}

impl fmt::Display for BenchRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let elapsed = Instant::now().duration_since(self.bench_begin);

        writeln!(
            f,
            "Elapsed {:?}, Total bytes: {}. Bytes per request: {:.3}. Per second: {:.3}",
            elapsed,
            self.total_bytes,
            self.total_bytes as f64 / self.total_requests as f64,
            self.total_bytes as f64 / elapsed.as_secs_f64()
        )?;

        if !self.summary.is_empty() {
            writeln!(f)?;

            let mut pairs: Vec<(String, i32)> =
                self.summary.iter().map(|(k, v)| (k.clone(), *v)).collect();

            pairs.sort_by(|a, b| {
                let d = b.1 - a.1;
                match d {
                    1..=0x7fffffff => cmp::Ordering::Greater,
                    0 => a.0.cmp(&b.0),
                    _ => cmp::Ordering::Less,
                }
            });

            writeln!(f, "Summary:")?;
            for pair in pairs {
                writeln!(f, "{}: {}", pair.0, pair.1)?;
            }
        }

        if self.latencies.entries() > 0 {
            writeln!(f)?;
            writeln!(
                f,
                "Percentiles: p50: {}µs p90: {}µs p99: {}µs p99.9: {}µs",
                self.latencies.percentile(50.0).unwrap(),
                self.latencies.percentile(90.0).unwrap(),
                self.latencies.percentile(99.0).unwrap(),
                self.latencies.percentile(99.9).unwrap(),
            )?;

            writeln!(
                f,
                "Latency (µs): Min: {}µs Avg: {}µs Max: {}µs StdDev: {}µs",
                self.latencies.minimum().unwrap(),
                self.latencies.mean().unwrap(),
                self.latencies.maximum().unwrap(),
                self.latencies.stddev().unwrap(),
            )
        } else {
            writeln!(f)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_session::BenchRun;

    #[test]
    fn test_codes() {
        let mut bench_run = BenchRun::new();
        bench_run.increment("200 OK".to_string());
        bench_run.increment("200 OK".to_string());
        bench_run.increment("400 BAD_REQUEST".to_string());
        bench_run.increment("502 BAD_GATEWAY".to_string());
        bench_run.increment("502 BAD_GATEWAY".to_string());
        bench_run.increment("502 BAD_GATEWAY".to_string());

        let as_str = bench_run.to_string();
        assert!(as_str.contains("400 BAD_REQUEST: 1"));
        assert!(as_str.contains("200 OK: 2"));
        assert!(as_str.contains("502 BAD_GATEWAY: 3"));
    }

    #[test]
    fn test_latencies() {
        let mut bench_run = BenchRun::new();
        for i in 0..1000 {
            bench_run.report_latency(i).expect("Shouldn't fail");
        }

        let as_str = bench_run.to_string();

        println!("{}", as_str);

        assert!(as_str.contains("p50: 500µs "));
        assert!(as_str.contains("p90: 900µs "));
        assert!(as_str.contains("p99: 990µs "));
        assert!(as_str.contains("p99.9: 999µs"));

        assert!(as_str.contains("Min: 0µs "));
        assert!(as_str.contains("Avg: 500µs "));
        assert!(as_str.contains("Max: 999µs "));
        assert!(as_str.contains("StdDev: 289µs"));
    }

    #[test]
    fn test_merge() {
        let mut b1 = BenchRun::new();
        let mut b2 = BenchRun::new();

        b1.total_bytes += 1;
        b2.total_bytes += 10;

        b1.total_requests += 1;
        b2.total_requests += 10;

        for i in 0..500 {
            b1.report_latency(i).expect("Shouldn't fail");
        }
        for i in 500..1000 {
            b2.report_latency(i).expect("Shouldn't fail");
        }

        b1.increment("200 OK".to_string());
        b2.increment("200 OK".to_string());
        b2.increment("400 BAD_REQUEST".to_string());
        b2.increment("502 BAD_GATEWAY".to_string());
        b1.increment("502 BAD_GATEWAY".to_string());
        b2.increment("502 BAD_GATEWAY".to_string());

        b1.merge(&b2);

        let as_str = b1.to_string();

        println!("{}", as_str);

        assert!(as_str.contains("Total bytes: 11."));
        assert!(as_str.contains("Bytes per request: 1.000."));

        assert!(as_str.contains("400 BAD_REQUEST: 1"));
        assert!(as_str.contains("200 OK: 2"));
        assert!(as_str.contains("502 BAD_GATEWAY: 3"));

        assert!(as_str.contains("p50: 500µs "));
        assert!(as_str.contains("p90: 900µs "));
        assert!(as_str.contains("p99: 990µs "));
        assert!(as_str.contains("p99.9: 999µs"));

        assert!(as_str.contains("Min: 0µs "));
        assert!(as_str.contains("Avg: 500µs "));
        assert!(as_str.contains("Max: 999µs "));
        assert!(as_str.contains("StdDev: 289µs"));
    }
}
