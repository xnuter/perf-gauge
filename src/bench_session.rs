use crate::bench_run::BenchRun;
use crate::configuration::BenchmarkMode;
use crate::metrics::{BenchRunMetrics, RequestStats};
use crate::rate_limiter::RateLimiter;
/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use derive_builder::Builder;
use log::error;
use log::info;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

#[derive(Builder, Clone)]
pub struct BenchSession {
    concurrency: usize,
    rate_ladder: RateLadder,
    mode: Arc<BenchmarkMode>,
    #[builder(setter(skip))]
    current_iteration: usize,
}

#[derive(Debug)]
pub struct BenchBatch {
    runs: Vec<BenchRun>,
    mode: Arc<BenchmarkMode>,
}

#[derive(Builder, Debug, Clone)]
pub struct RateLadder {
    start: f64,
    end: f64,
    rate_increment: Option<f64>,
    step_duration: Option<Duration>,
    step_requests: Option<usize>,
    #[builder(default = "0.0")]
    current: f64,
    #[builder(default = "1")]
    max_rate_iterations: usize,
    #[builder(default = "false")]
    complete: bool,
}

impl Iterator for BenchSession {
    type Item = BenchBatch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rate_ladder.complete {
            return None;
        }

        let current = self.rate_ladder.get_current();

        let mut items = vec![];

        let rate_per_second = current / self.concurrency as f64;

        for i in 0..self.concurrency {
            let idx = i + self.current_iteration * self.concurrency;
            items.push(if let Some(requests) = self.rate_ladder.step_requests {
                BenchRun::with_request_limit(
                    idx,
                    requests,
                    RateLimiter::build_rate_limiter(rate_per_second),
                )
            } else if let Some(duration) = self.rate_ladder.step_duration {
                BenchRun::with_duration_limit(
                    idx,
                    duration,
                    RateLimiter::build_rate_limiter(rate_per_second),
                )
            } else {
                unreachable!();
            });
        }

        self.rate_ladder.increment_rate();
        self.current_iteration += 1;

        Some(BenchBatch {
            runs: items,
            mode: self.mode.clone(),
        })
    }
}

impl BenchBatch {
    pub async fn run(self, mut metrics: BenchRunMetrics) -> Result<BenchRunMetrics, String> {
        let (metrics_sender, mut metrics_receiver) = mpsc::channel(1_000);

        // single consumer to aggregate metrics
        let metrics_aggregator = tokio::spawn(async move {
            while let Some(request_stats) = metrics_receiver.recv().await {
                metrics.report_request(request_stats);
            }
            metrics
        });

        // while there are going to be multiple metrics producers
        self.execute_concurrent_sessions(metrics_sender).await?;

        Ok(metrics_aggregator
            .await
            .expect("Must return metrics object at the end"))
    }

    async fn execute_concurrent_sessions(
        self,
        metrics_sender: Sender<RequestStats>,
    ) -> Result<(), String> {
        let mut concurrent_clients = vec![];
        for bench_run in self.runs.into_iter() {
            let bench_protocol_adapter = self.mode.clone();
            let metrics_channel = metrics_sender.clone();
            concurrent_clients.push(tokio::spawn(async move {
                bench_run
                    .send_load(
                        match bench_protocol_adapter.as_ref() {
                            BenchmarkMode::Http(http_bench_session) => http_bench_session,
                        },
                        metrics_channel,
                    )
                    .await
            }));
        }

        for t in concurrent_clients.into_iter() {
            t.await.map_err(|e| {
                error!("Cannot join bench run. Error: {}", e);
                e.to_string()
            })??;
        }

        Ok(())
    }
}

impl RateLadder {
    fn get_current(&self) -> f64 {
        self.current.max(self.start)
    }

    fn increment_rate(&mut self) {
        debug_assert!(
            !self.complete,
            "Method shouldn't be called if it's complete"
        );

        match self.rate_increment {
            None if self.max_rate_iterations <= 1 => {
                self.complete = true;
            }
            None => {
                self.max_rate_iterations -= 1;
            }
            Some(rate_increment) => {
                let distance_to_end = self.end - self.get_current();
                let increment = rate_increment.min(distance_to_end);
                if increment < 1. {
                    if self.max_rate_iterations > 0 {
                        self.max_rate_iterations -= 1;
                        info!(
                            "Max run rate: {}. Iterations left: {}",
                            self.current, self.max_rate_iterations
                        );
                    } else {
                        self.complete = true;
                    }
                };
                self.current = self.get_current() + increment;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_session::RateLadderBuilder;

    #[test]
    fn test_rate_ladder_with_increment() {
        let mut rate_ladder = RateLadderBuilder::default()
            .start(5000.)
            .end(10000.)
            .rate_increment(Some(5000.))
            .step_duration(None)
            .step_requests(None)
            .max_rate_iterations(0)
            .build()
            .expect("Failed to build");

        assert_eq!(5000., rate_ladder.get_current());

        rate_ladder.increment_rate();
        assert_eq!(10000., rate_ladder.get_current());

        // done
        rate_ladder.increment_rate();
        assert_eq!(10000., rate_ladder.get_current());
        assert!(rate_ladder.complete);
    }

    #[test]
    fn test_rate_ladder_with_max_reps() {
        let max_rate_reps = 10;
        let max_rate = 10000.;
        let start = 5000.;
        let mut rate_ladder = RateLadderBuilder::default()
            .start(start)
            .end(max_rate)
            .rate_increment(Some(start))
            .step_duration(None)
            .step_requests(None)
            .max_rate_iterations(max_rate_reps)
            .build()
            .expect("Failed to build");

        assert_eq!(start, rate_ladder.get_current());

        rate_ladder.increment_rate();
        assert_eq!(max_rate, rate_ladder.get_current());

        for _ in 0..max_rate_reps {
            rate_ladder.increment_rate();
            assert_eq!(max_rate, rate_ladder.get_current());
        }

        // done
        rate_ladder.increment_rate();
        assert_eq!(max_rate, rate_ladder.get_current());
        assert!(rate_ladder.complete);
    }

    #[test]
    fn test_rate_ladder_without_increment() {
        let start = 5000.;
        let end = 10000.;
        let mut rate_ladder = RateLadderBuilder::default()
            .start(start)
            .end(end)
            .rate_increment(None)
            .step_duration(None)
            .step_requests(None)
            .build()
            .expect("Failed to build");

        assert_eq!(start, rate_ladder.get_current());
        assert!(!rate_ladder.complete);
        rate_ladder.increment_rate();
        assert_eq!(start, rate_ladder.get_current());
        assert!(rate_ladder.complete);
    }

    #[test]
    fn test_rate_ladder_without_increment_multiple_iterations() {
        let start = 5000.;
        let end = 10000.;
        let max_iterations = 100;
        let mut rate_ladder = RateLadderBuilder::default()
            .start(start)
            .end(end)
            .rate_increment(None)
            .step_duration(None)
            .step_requests(None)
            .max_rate_iterations(max_iterations)
            .build()
            .expect("Failed to build");

        assert_eq!(start, rate_ladder.get_current());
        assert!(!rate_ladder.complete);

        for i in 0..max_iterations {
            rate_ladder.increment_rate();
            assert_eq!(start, rate_ladder.get_current());
            assert_eq!(i + 1 == max_iterations, rate_ladder.complete);
        }
    }
}
