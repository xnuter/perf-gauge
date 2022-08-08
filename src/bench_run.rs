use crate::metrics::RequestStats;
use crate::rate_limiter::RateLimiter;
/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use async_trait::async_trait;
use log::error;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::time::timeout;

static STOP_ON_FATAL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct BenchRun {
    pub index: usize,
    bench_begin: Instant,
    timeout: Option<Duration>,
    requests_sent: usize,
    max_requests: Option<usize>,
    max_duration: Option<Duration>,
    rate_limiter: RateLimiter,
}

#[async_trait]
pub trait BenchmarkProtocolAdapter {
    type Client;

    fn build_client(&self) -> Result<Self::Client, String>;
    async fn send_request(&self, client: &Self::Client) -> RequestStats;
}

impl BenchRun {
    pub fn from_request_limit(
        index: usize,
        max_requests: usize,
        rate_limiter: RateLimiter,
        timeout: Option<Duration>,
    ) -> Self {
        Self::new(index, Some(max_requests), None, rate_limiter, timeout)
    }

    pub fn from_duration_limit(
        index: usize,
        max_duration: Duration,
        rate_limiter: RateLimiter,
        timeout: Option<Duration>,
    ) -> Self {
        Self::new(index, None, Some(max_duration), rate_limiter, timeout)
    }

    fn new(
        index: usize,
        max_requests: Option<usize>,
        max_duration: Option<Duration>,
        rate_limiter: RateLimiter,
        timeout: Option<Duration>,
    ) -> Self {
        assert!(
            max_duration.is_some() || max_requests.is_some(),
            "Bug: bench run should limited either by duration or number of requests"
        );

        Self {
            index,
            bench_begin: Instant::now(),
            timeout,
            requests_sent: 0,
            max_requests,
            max_duration,
            rate_limiter,
        }
    }

    pub fn has_more_work(&mut self) -> bool {
        let has_more_work = if let Some(max_requests) = self.max_requests {
            self.requests_sent < max_requests
        } else if let Some(max_duration) = self.max_duration {
            Instant::now().duration_since(self.bench_begin) < max_duration
        } else {
            unreachable!();
        };

        self.requests_sent += 1;

        has_more_work
    }

    pub async fn send_load(
        mut self,
        bench_protocol_adapter: &impl BenchmarkProtocolAdapter,
        metrics_channel: Sender<RequestStats>,
    ) -> Result<(), String> {
        let client = bench_protocol_adapter.build_client()?;

        while self.has_more_work() {
            self.rate_limiter.acquire_one().await;

            if STOP_ON_FATAL.load(Ordering::Relaxed) {
                break;
            }

            let timed_request = self
                .timed_operation(bench_protocol_adapter.send_request(&client))
                .await;

            let fatal_error = match timed_request {
                Ok(request_stats) => {
                    let failed = request_stats.fatal_error;
                    metrics_channel
                        .try_send(request_stats)
                        .map_err(|e| {
                            error!("Error sending metrics: {}", e);
                        })
                        .unwrap_or_default();
                    failed
                }
                Err(_) => true,
            };

            if fatal_error {
                STOP_ON_FATAL.store(true, Ordering::Relaxed);
                break;
            }
        }

        Ok(())
    }

    /// Each async operation must be time-bound.
    pub async fn timed_operation<T: Future>(&self, f: T) -> Result<<T as Future>::Output, ()> {
        if let Some(timeout_value) = self.timeout {
            let result = timeout(timeout_value, f).await;

            if let Ok(r) = result {
                Ok(r)
            } else {
                Err(())
            }
        } else {
            Ok(f.await)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_run::STOP_ON_FATAL;
    use crate::bench_session::RateLadderBuilder;
    use crate::configuration::BenchmarkMode::Http;
    use crate::configuration::{BenchmarkConfig, BenchmarkConfigBuilder};
    use crate::http_bench_session::{
        HttpBenchAdapterBuilder, HttpClientConfigBuilder, HttpRequestBuilder,
    };
    use crate::metrics::BenchRunMetrics;
    use mockito::mock;
    use std::sync::atomic::Ordering;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn test_send_load() {
        let body = "world";

        let request_count = 100;

        let _m = mock("GET", "/1")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .expect(request_count)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_adapter = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{}/1", url)])
                    .build()
                    .unwrap(),
            )
            .config(
                HttpClientConfigBuilder::default()
                    .stop_on_errors(vec![401])
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let benchmark_config: BenchmarkConfig = BenchmarkConfigBuilder::default()
            .rate_ladder(
                RateLadderBuilder::default()
                    .start(request_count as f64)
                    .end(request_count as f64)
                    .rate_increment(None)
                    .step_duration(None)
                    .step_requests(Some(request_count))
                    .build()
                    .expect("RateLadderBuilder failed"),
            )
            .mode(Http(http_adapter.clone()))
            .request_timeout(None)
            .build()
            .expect("BenchmarkConfig failed");

        let start = Instant::now();

        let mut session = benchmark_config.clone().new_bench_session();

        let bench_run_stats = BenchRunMetrics::new();

        STOP_ON_FATAL.store(false, Ordering::Relaxed);

        let bench_result = session
            .next()
            .expect("Must have runs")
            .run(bench_run_stats)
            .await;

        assert!(!STOP_ON_FATAL.load(Ordering::Relaxed));
        assert!(bench_result.is_ok());

        let elapsed = Instant::now().duration_since(start).as_secs_f64();
        let time_delta = (elapsed - 1.).abs();
        assert!(
            time_delta < 0.3,
            "Expected to finish in ~1s, but it took: {}",
            elapsed
        );

        let stats = bench_result.unwrap();

        assert_eq!(body.len() * request_count, stats.combined.total_bytes);
        assert_eq!(request_count, stats.combined.total_requests);
        assert_eq!(
            stats.combined.summary.get("200 OK"),
            Some(&(request_count as i32))
        );
    }

    #[tokio::test]
    async fn test_send_load_fatal_code() {
        let body = "world";

        let request_count = 100;

        let _m = mock("GET", "/1")
            .with_status(401)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .expect(request_count)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_adapter = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{}/1", url)])
                    .build()
                    .unwrap(),
            )
            .config(
                HttpClientConfigBuilder::default()
                    .stop_on_errors(vec![401])
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let benchmark_config: BenchmarkConfig = BenchmarkConfigBuilder::default()
            .rate_ladder(
                RateLadderBuilder::default()
                    .start(request_count as f64)
                    .end(request_count as f64)
                    .rate_increment(None)
                    .step_duration(None)
                    .step_requests(Some(request_count))
                    .build()
                    .expect("RateLadderBuilder failed"),
            )
            .request_timeout(None)
            .mode(Http(http_adapter.clone()))
            .build()
            .expect("BenchmarkConfig failed");

        let mut session = benchmark_config.clone().new_bench_session();

        let bench_run_stats = BenchRunMetrics::new();

        STOP_ON_FATAL.store(false, Ordering::Relaxed);

        let bench_result = session
            .next()
            .expect("Must have runs")
            .run(bench_run_stats)
            .await;

        // must stop on fatal
        assert!(STOP_ON_FATAL.load(Ordering::Relaxed));
        assert!(bench_result.is_ok());
    }

    #[tokio::test]
    async fn test_send_load_with_timeout() {
        let request_count = 100;

        let _m = mock("GET", "/1")
            .with_status(200)
            .with_body_from_fn(|_| {
                sleep(Duration::from_secs(10));
                Ok(())
            })
            .with_header("content-type", "text/plain")
            .expect(request_count)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_adapter = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{}/1", url)])
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let benchmark_config: BenchmarkConfig = BenchmarkConfigBuilder::default()
            .rate_ladder(
                RateLadderBuilder::default()
                    .start(request_count as f64)
                    .end(request_count as f64)
                    .rate_increment(None)
                    .step_duration(None)
                    .step_requests(Some(request_count))
                    .build()
                    .expect("RateLadderBuilder failed"),
            )
            .request_timeout(Some(Duration::from_millis(10)))
            .mode(Http(http_adapter.clone()))
            .build()
            .expect("BenchmarkConfig failed");

        let mut session = benchmark_config.clone().new_bench_session();

        let bench_run_stats = BenchRunMetrics::new();

        STOP_ON_FATAL.store(false, Ordering::Relaxed);

        let bench_result = session
            .next()
            .expect("Must have runs")
            .run(bench_run_stats)
            .await;

        // must stop on timeout, treated as fatal
        assert!(STOP_ON_FATAL.load(Ordering::Relaxed));
        assert!(bench_result.is_ok());
    }
}
