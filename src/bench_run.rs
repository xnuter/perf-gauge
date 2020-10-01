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
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub struct BenchRun {
    pub index: usize,
    bench_begin: Instant,
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
    pub fn with_request_limit(
        index: usize,
        max_requests: usize,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self::new(index, Some(max_requests), None, rate_limiter)
    }

    pub fn with_duration_limit(
        index: usize,
        max_duration: Duration,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self::new(index, None, Some(max_duration), rate_limiter)
    }

    fn new(
        index: usize,
        max_requests: Option<usize>,
        max_duration: Option<Duration>,
        rate_limiter: RateLimiter,
    ) -> Self {
        assert!(
            max_duration.is_some() || max_requests.is_some(),
            "Bug: bench run should limited either by duration or number of requests"
        );

        Self {
            index,
            bench_begin: Instant::now(),
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
        mut metrics_channel: Sender<RequestStats>,
    ) -> Result<(), String> {
        let client = bench_protocol_adapter.build_client()?;

        while self.has_more_work() {
            self.rate_limiter
                .acquire_one()
                .await
                .expect("Unexpected LeakyBucket.acquire error");

            metrics_channel
                .try_send(bench_protocol_adapter.send_request(&client).await)
                .map_err(|e| {
                    error!("Error sending metrics: {}", e);
                })
                .unwrap_or_default();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_session::RateLadderBuilder;
    use crate::configuration::BenchmarkMode::HTTP;
    use crate::configuration::{BenchmarkConfig, BenchmarkConfigBuilder};
    use crate::http_bench_session::HttpBenchAdapterBuilder;
    use crate::metrics::BenchRunMetrics;
    use mockito::mock;
    use std::time::Instant;

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
            .url(format!("{}/1", url))
            .tunnel(None)
            .ignore_cert(false)
            .conn_reuse(false)
            .store_cookies(false)
            .http2_only(false)
            .verbose(true)
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
            .concurrency(1)
            .verbose(false)
            .mode(HTTP(http_adapter.clone()))
            .reporters(vec![])
            .build()
            .expect("BenchmarkConfig failed");

        let start = Instant::now();

        let mut session = benchmark_config.clone().new_bench_session();

        let bench_run_stats = BenchRunMetrics::new();

        let bench_result = session
            .next()
            .expect("Must have runs")
            .run(bench_run_stats)
            .await;

        assert!(bench_result.is_ok());

        let elapsed = Instant::now().duration_since(start).as_secs_f64();
        let time_delta = (elapsed - 1.).abs();
        assert!(
            time_delta < 0.2,
            "Expected to finish in ~1s, but it took: {}",
            elapsed
        );

        let stats = bench_result.unwrap();

        assert_eq!(body.len() * request_count, stats.total_bytes);
        assert_eq!(request_count, stats.total_requests);
        assert_eq!(stats.summary.get("200 OK"), Some(&(request_count as i32)));
    }
}
