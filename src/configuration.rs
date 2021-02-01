use crate::bench_session::{BenchSession, BenchSessionBuilder, RateLadder, RateLadderBuilder};
/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::http_bench_session::{HttpBenchAdapter, HttpBenchAdapterBuilder};
use crate::metrics::{DefaultConsoleReporter, ExternalMetricsServiceReporter};
use crate::prometheus_reporter::PrometheusReporter;
use clap::{clap_app, ArgMatches};
use core::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io;

#[derive(Clone, Debug)]
pub enum BenchmarkMode {
    Http(HttpBenchAdapter),
}

#[derive(Clone, Builder)]
pub struct BenchmarkConfig {
    #[builder(default)]
    pub name: Option<String>,
    #[builder(default)]
    pub verbose: bool,
    #[builder(default = "1")]
    pub concurrency: usize,
    pub rate_ladder: RateLadder,
    pub mode: BenchmarkMode,
    #[builder(default)]
    pub reporters: Vec<Arc<Box<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>>>,
}

impl BenchmarkConfig {
    pub fn from_command_line() -> io::Result<BenchmarkConfig> {
        let matches = clap_app!(myapp =>
            (name: "Performance Gauge")
            (version: "0.1.0")
            (author: "Eugene Retunsky")
            (about: "A tool for gauging performance of network services")
            (@arg CONCURRENCY: --concurrency -c +takes_value "Concurrent clients. Default `1`.")
            (@group duration =>
                (@arg NUMBER_OF_REQUESTS: --num_req -n +takes_value "Number of requests.")
                (@arg DURATION: --duration -d +takes_value "Duration of the test.")
            )
            (@arg TEST_CASE_NAME: --name -N +takes_value "Test case name. Optional. Can be used for tagging metrics.")
            (@arg RATE: --rate -r +takes_value "Request rate per second. E.g. 100 or 0.1. By default no limit.")
            (@arg RATE_STEP: --rate_step +takes_value "Rate increase step (until it reaches --rate_max).")
            (@arg RATE_MAX: --rate_max +takes_value "Max rate per second. Requires --rate-step")
            (@arg MAX_RATE_ITERATIONS: --max_iter -m +takes_value "The number of iterations with the max rate. By default `1`.")
            (@arg PROMETHEUS_ADDR: --prometheus +takes_value "If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g. 10.0.0.1:9091")
            (@arg PROMETHEUS_JOB: --prometheus_job +takes_value "Prometheus Job (by default `pushgateway`)")
            (@subcommand http =>
                (about: "Run in HTTP(S) mode")
                (version: "0.1.0")
                (@arg TUNNEL: --tunnel +takes_value "HTTP Tunnel used for connection, e.g. http://my-proxy.org")
                (@arg IGNORE_CERT: --ignore_cert "Allow self signed certificates. Applies to the target (not proxy).")
                (@arg CONN_REUSE: --conn_reuse "If connections should be re-used")
                (@arg STORE_COOKIES: --store_cookies "If cookies should be stored")
                (@arg HTTP2_ONLY: --http2_only "Enforce HTTP/2 only")
                (@arg TARGET: +required ... "Target, e.g. https://my-service.com:8443/8kb Can be multiple ones (with random choice balancing)")
                (@arg METHOD: --method -M +takes_value "Method. By default GET")
                (@arg HEADER: --header -H ... "Headers in \"Name:Value\" form. Can be provided multiple times.")
                (@arg BODY: --body -B  +takes_value "Body of the request in base64. Optional.")
            )
        ).get_matches();

        let test_case_name = matches.value_of("TEST_CASE_NAME").map(|s| s.to_string());
        let concurrency = matches.value_of("CONCURRENCY").unwrap_or("1");
        let rate_per_second = matches.value_of("RATE");
        let rate_step = matches.value_of("RATE_STEP");
        let rate_max = matches.value_of("RATE_MAX");
        let max_rate_iterations = matches.value_of("MAX_RATE_ITERATIONS").unwrap_or("1");

        let duration = matches.value_of("DURATION").map(|d| {
            humantime::Duration::from_str(d)
                .expect("Illegal duration")
                .into()
        });

        let number_of_requests = matches
            .value_of("NUMBER_OF_REQUESTS")
            .map(|n| parse_num(n, "Illegal number for NUMBER_OF_REQUESTS"));

        let rate_ladder = if let Some(rate_max) = rate_max {
            let rate_per_second =
                rate_per_second.expect("RATE is required if RATE_MAX is specified");
            let rate_step = rate_step.expect("RATE_STEP is required if RATE_MAX is specified");
            RateLadderBuilder::default()
                .start(parse_num(rate_per_second, "Cannot parse RATE"))
                .end(parse_num(rate_max, "Cannot parse RATE_MAX"))
                .rate_increment(Some(parse_num(rate_step, "Cannot parse RATE_STEP")))
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(parse_num(
                    max_rate_iterations,
                    "Cannot parse MAX_RATE_ITERATIONS",
                ))
                .build()
                .expect("RateLadderBuilder failed")
        } else {
            let rps = parse_num(rate_per_second.unwrap_or("0"), "Cannot parse RATE");
            RateLadderBuilder::default()
                .start(rps)
                .end(rps)
                .rate_increment(None)
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(parse_num(
                    max_rate_iterations,
                    "Cannot parse MAX_RATE_ITERATIONS",
                ))
                .build()
                .expect("RateLadderBuilder failed")
        };

        let mut metrics_destinations: Vec<
            Arc<Box<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>>,
        > = vec![Arc::new(Box::new(DefaultConsoleReporter::new(
            test_case_name.clone(),
        )))];

        if let Some(prometheus_addr) = matches.value_of("PROMETHEUS_ADDR") {
            if SocketAddr::from_str(prometheus_addr).is_err() {
                panic!("Illegal Prometheus Gateway addr `{}`", prometheus_addr);
            }
            metrics_destinations.push(Arc::new(Box::new(PrometheusReporter::new(
                test_case_name.clone(),
                prometheus_addr.to_string(),
                matches.value_of("PROMETHEUS_JOB"),
            ))));
        }

        Ok(BenchmarkConfigBuilder::default()
            .name(test_case_name)
            .rate_ladder(rate_ladder)
            .concurrency(parse_num(concurrency, "Cannot parse CONCURRENCY"))
            .verbose(false)
            .mode(BenchmarkConfig::build_mode(&matches))
            .reporters(metrics_destinations)
            .build()
            .expect("BenchmarkConfig failed"))
    }

    fn build_mode(matches: &ArgMatches) -> BenchmarkMode {
        let mode = if let Some(config) = matches.subcommand_matches("http") {
            let http_config = HttpBenchAdapterBuilder::default()
                .url(
                    config
                        .values_of("TARGET")
                        .expect("misconfiguration for TARGET")
                        .map(|s| s.to_string())
                        .collect(),
                )
                .tunnel(config.value_of("TUNNEL").map(|s| s.to_string()))
                .ignore_cert(config.is_present("IGNORE_CERT"))
                .conn_reuse(config.is_present("CONN_REUSE"))
                .store_cookies(config.is_present("STORE_COOKIES"))
                .http2_only(config.is_present("HTTP2_ONLY"))
                .method(config.value_of("METHOD").unwrap_or("GET").to_string())
                .headers(BenchmarkConfig::get_multiple_values(config, "HEADER"))
                .body(
                    config
                        .value_of("BODY")
                        .map(|s| base64::decode(s).expect("Invalid base64"))
                        .unwrap_or_else(Vec::new),
                )
                .build()
                .expect("BenchmarkModeBuilder failed");
            BenchmarkMode::Http(http_config)
        } else {
            unreachable!("Unknown subcommand: {}", matches.subcommand().unwrap().0);
        };
        mode
    }

    fn get_multiple_values(config: &ArgMatches, id: &str) -> Vec<(String, String)> {
        config
            .values_of(id)
            .map(|v| {
                v.map(|s| {
                    let mut split = s.split(':');
                    (
                        split.next().expect("Header name is missing").to_string(),
                        split.collect::<Vec<&str>>().join(":"),
                    )
                })
                .collect()
            })
            .unwrap_or_else(Vec::new)
    }

    pub fn new_bench_session(&mut self) -> BenchSession {
        BenchSessionBuilder::default()
            .concurrency(self.concurrency)
            .rate_ladder(self.rate_ladder.clone())
            .mode(Arc::new(self.mode.clone()))
            .build()
            .expect("BenchSessionBuilder failed")
    }
}

impl fmt::Display for BenchmarkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Mode={:?}, RateLadder={:?}, Concurrency={}",
            self.mode, self.rate_ladder, self.concurrency
        )
    }
}

pub fn parse_num<F: FromStr>(s: &str, error_msg: &str) -> F {
    s.parse()
        .map_err(|_| {
            println!("{}", error_msg);
            panic!("Cannot start");
        })
        .unwrap()
}
