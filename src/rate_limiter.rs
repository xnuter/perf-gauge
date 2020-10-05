/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use leaky_bucket::{LeakyBucket, LeakyBuckets};
use log::{error, info};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct RateLimiter {
    leaky_bucket: Option<LeakyBucket>,
}

impl RateLimiter {
    pub fn build_rate_limiter(rate_per_second: f64) -> RateLimiter {
        if rate_per_second == 0. {
            // unlimited
            return RateLimiter { leaky_bucket: None };
        }

        let (amount, interval) = if rate_per_second > 1. {
            let mut rate = rate_per_second;
            let mut int_ms = 1000;
            while int_ms > 10 && rate > 10. {
                rate /= 10.;
                int_ms /= 10;
            }
            (rate, Duration::from_millis(int_ms))
        } else {
            (
                1.,
                Duration::from_millis((1. / rate_per_second * 1000.) as u64),
            )
        };

        info!("Rate limiter: {} per {:?}", amount, interval);

        let mut buckets = LeakyBuckets::new();
        let coordinator = buckets.coordinate().expect("no other running coordinator");
        tokio::spawn(async move {
            match coordinator.await {
                Ok(_) => {
                    info!("Rate limiter is done");
                }
                Err(e) => {
                    error!("Rate limiter crashed: {}", e);
                }
            }
        });

        RateLimiter {
            leaky_bucket: Some(
                buckets
                    .rate_limiter()
                    // to compensate overhead let's add a bit to the rate
                    .refill_amount((amount * 1.01) as usize)
                    .refill_interval(interval)
                    .build()
                    .expect("LeakyBucket builder failed"),
            ),
        }
    }

    pub async fn acquire_one(&self) -> Result<(), String> {
        match self.leaky_bucket.as_ref() {
            None => Ok(()),
            Some(leaky_bucket) => leaky_bucket.acquire_one().await.map_err(|e| {
                error!("Error acquiring permit: {}", e);
                e.to_string()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rate_limiter::RateLimiter;
    use std::time::Instant;

    #[tokio::test]
    async fn test_limited_frequent() {
        let rate_limiter = RateLimiter::build_rate_limiter(100.);
        let begin = Instant::now();
        for _ in 0..100 {
            rate_limiter.acquire_one().await.expect("No reason to fail");
        }
        let elapsed = Instant::now().duration_since(begin);
        println!("Elapsed: {:?}", elapsed);
        assert!((elapsed.as_secs_f64() - 1.).abs() < 0.2);
    }

    #[tokio::test]
    async fn test_limited_seldom() {
        let rate_limiter = RateLimiter::build_rate_limiter(0.5);
        let begin = Instant::now();
        for _ in 0..2 {
            rate_limiter.acquire_one().await.expect("No reason to fail");
        }
        let elapsed = Instant::now().duration_since(begin);
        println!("Elapsed: {:?}", elapsed);
        // once per 2 seconds => 4 seconds for 2 permits
        assert!((elapsed.as_secs_f64() - 4.).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_unlimited() {
        let rate_limiter = RateLimiter::build_rate_limiter(0.);
        let begin = Instant::now();
        for _ in 0..1_000_000 {
            rate_limiter.acquire_one().await.expect("No reason to fail");
        }
        let elapsed = Instant::now().duration_since(begin);
        println!("Elapsed: {:?}", elapsed);
        assert!(elapsed.as_secs_f64() < 1.);
    }
}
