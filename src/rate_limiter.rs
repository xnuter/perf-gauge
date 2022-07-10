/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use leaky_bucket::RateLimiter as InnerRateLimiter;
use log::debug;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct RateLimiter {
    leaky_bucket: Option<Arc<InnerRateLimiter>>,
}

impl RateLimiter {
    pub fn build_rate_limiter(rate_per_second: f64) -> RateLimiter {
        if rate_per_second == 0. {
            // unlimited
            return RateLimiter { leaky_bucket: None };
        }

        let (amount, interval) = RateLimiter::rate_to_refill_amount_and_duration(rate_per_second);

        debug!(
            "Rate limiter: {} per {:?}. Per second: {}",
            amount,
            interval,
            amount / interval.as_secs_f64()
        );

        RateLimiter {
            leaky_bucket: Some(Arc::new(
                InnerRateLimiter::builder()
                    // to compensate overhead let's add a bit to the rate
                    .refill((amount * 1.01) as usize)
                    .interval(interval)
                    .max(amount as usize * 100)
                    .build(),
            )),
        }
    }

    pub async fn acquire_one(&self) {
        if let Some(leaky_bucket) = self.leaky_bucket.as_ref() {
            leaky_bucket.acquire_one().await;
        }
    }

    fn gcd(mut a: usize, mut b: usize) -> usize {
        while b != 0 {
            let t = b;
            b = a % b;
            a = t;
        }
        a
    }

    fn rate_to_refill_amount_and_duration(rate_per_second: f64) -> (f64, Duration) {
        if rate_per_second > 1. {
            let mut rate = rate_per_second as usize;
            let mut int_ms = 1000;

            let gcd = RateLimiter::gcd(rate, int_ms);
            rate /= gcd;
            int_ms /= gcd;

            (rate as f64, Duration::from_millis(int_ms as u64))
        } else {
            (
                1.,
                Duration::from_millis((1. / rate_per_second * 1000.) as u64),
            )
        }
    }
}

impl fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimiter").finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::rate_limiter::RateLimiter;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn test_limited_frequent() {
        let rate_limiter = RateLimiter::build_rate_limiter(100.);
        let begin = Instant::now();
        for _ in 0..100 {
            rate_limiter.acquire_one().await;
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
            rate_limiter.acquire_one().await;
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
            rate_limiter.acquire_one().await;
        }
        let elapsed = Instant::now().duration_since(begin);
        println!("Elapsed: {:?}", elapsed);
        assert!(elapsed.as_secs_f64() < 1.);
    }

    #[test]
    fn test_rate_to_refill_amount_and_duration() {
        let test = vec![
            (0.1, (1., Duration::from_secs(10))),
            (0.5, (1., Duration::from_secs(2))),
            (1., (1., Duration::from_secs(1))),
            (2., (1., Duration::from_millis(500))),
            (5., (1., Duration::from_millis(200))),
            (100., (1., Duration::from_millis(10))),
            (150., (3., Duration::from_millis(20))),
            (250., (1., Duration::from_millis(4))),
            (300., (3., Duration::from_millis(10))),
            (1000., (1., Duration::from_millis(1))),
            (1250., (5., Duration::from_millis(4))),
            (1500., (3., Duration::from_millis(2))),
            (2000., (2., Duration::from_millis(1))),
            (2222., (1111., Duration::from_millis(500))),
            (5000., (5., Duration::from_millis(1))),
        ];

        test.iter().for_each(|(rate, (amount, duration))| {
            assert_eq!(
                RateLimiter::rate_to_refill_amount_and_duration(*rate),
                (*amount, *duration)
            );
        });
    }
}
