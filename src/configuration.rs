/// Copyright 2020 Developers of the service-benchmark project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::http_bench_session::{HttpBenchAdapter, HttpBenchAdapterBuilder};
use clap::clap_app;
use leaky_bucket::LeakyBucket;
use log::info;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io;

#[derive(Clone, Debug)]
pub enum BenchmarkMode {
    HTTP(HttpBenchAdapter),
}

#[derive(Clone, Builder, Debug)]
pub struct BenchmarkConfig {
    pub verbose: bool,
    pub number_of_requests: usize,
    pub concurrency: usize,
    /// Rate limiting is global per app, so we share the same instance across multiple threads.
    pub rate_limiter: Arc<LeakyBucket>,
    pub mode: BenchmarkMode,
}

impl BenchmarkConfig {
    pub fn from_command_line() -> io::Result<BenchmarkConfig> {
        let matches = clap_app!(myapp =>
            (name: "Service benchmark")
            (version: "0.1.0")
            (author: "Eugene Retunsky")
            (about: "A benchmarking tool for network services")
            (@arg CONCURRENCY: --concurrency -c +takes_value "Concurrent threads. Default `1`.")
            (@arg NUMBER_OF_REQUESTS: --num_req -n +required +takes_value "Number of requests.")
            (@arg RATE_PER_SECOND: --rate-per-second -r +takes_value "Request rate per second. E.g. 100 or 0.1. By default no limit.")
            (@arg VERBOSE: --verbose -v "Print debug information. Not recommended for `-n > 500`")
            (@subcommand http =>
                (about: "Run in HTTP(S) mode")
                (version: "0.1.0")
                (@arg TUNNEL: --tunnel +takes_value "HTTP Tunnel used for connection, e.g. http://my-proxy.org")
                (@arg TARGET: +required "Target, e.g. https://my-service.com:8443/8kb")
                (@arg IGNORE_CERT: --ignore_cert "Allow self signed certificates. Applies to the target (not proxy).")
                (@arg CONN_REUSE: --conn_reuse "If connections should be re-used")
                (@arg STORE_COOKIES: --store_cookies "If cookies should be stored")
                (@arg HTTP2_ONLY: --http2_only "Enforce HTTP/2 only")
            )
        )
        .get_matches();

        let number_of_requests = matches
            .value_of("NUMBER_OF_REQUESTS")
            .expect("misconfiguration for NUMBER_OF_REQUESTS");
        let concurrency = matches.value_of("CONCURRENCY").unwrap_or("1");
        let rate_per_second = matches.value_of("RATE_PER_SECOND").unwrap_or("0");
        let verbose = matches.is_present("VERBOSE");

        let mode = if let Some(config) = matches.subcommand_matches("http") {
            let http_config = HttpBenchAdapterBuilder::default()
                .url(
                    config
                        .value_of("TARGET")
                        .expect("misconfiguration for TARGET")
                        .to_string(),
                )
                .tunnel(config.value_of("TUNNEL").map(|s| s.to_string()))
                .ignore_cert(config.is_present("IGNORE_CERT"))
                .conn_reuse(config.is_present("CONN_REUSE"))
                .store_cookies(config.is_present("STORE_COOKIES"))
                .http2_only(config.is_present("HTTP2_ONLY"))
                .verbose(verbose)
                .build()
                .expect("BenchmarkModeBuilder failed");
            BenchmarkMode::HTTP(http_config)
        } else {
            unreachable!("Unknown subcommand: {}", matches.subcommand().unwrap().0);
        };

        let rate_limit: f64 = parse_num(rate_per_second);

        Ok(BenchmarkConfigBuilder::default()
            .rate_limiter(build_rate_limiter(rate_limit))
            .number_of_requests(parse_num(number_of_requests))
            .concurrency(parse_num(concurrency))
            .verbose(verbose)
            .mode(mode)
            .build()
            .expect("BenchmarkConfig failed"))
    }
}

fn parse_num<F: FromStr>(s: &str) -> F {
    s.parse()
        .map_err(|_| panic!("Cannot parse: {}", s))
        .unwrap()
}

fn build_rate_limiter(rate_per_second: f64) -> Arc<LeakyBucket> {
    let (amount, interval) = if rate_per_second == 0. {
        // unlimited, or 1B per second
        (1_000_000., Duration::from_millis(1))
    } else if rate_per_second > 1. {
        let mut rate = rate_per_second;
        let mut int_ms = 1000;
        while int_ms >= 10 && rate >= 10. {
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

    Arc::new(
        LeakyBucket::builder()
            .refill_amount(amount as usize)
            .refill_interval(interval)
            .build()
            .expect("LeakyBucket builder failed"),
    )
}
